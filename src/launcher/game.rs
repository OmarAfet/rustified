use anyhow::{Context, Result, anyhow};
use log::{debug, info};
use std::env;
use std::process::{Command, Stdio};

use crate::auth::AuthResult;
use crate::launcher::get_library_path;
use crate::launcher::instance::InstanceConfig;
use crate::launcher::java::JavaManager;
use crate::launcher::minecraft_dir::MinecraftDir;
use crate::launcher::version::{ArgumentValue, ArgumentValueType, VersionInfo};

pub struct GameLauncher {}

impl GameLauncher {
    pub fn new() -> Self {
        Self {}
    }

    /// Launch the Minecraft game with a specific instance
    pub async fn launch(
        &self,
        version_info: &VersionInfo,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        java_manager: &JavaManager,
        instance: Option<&InstanceConfig>,
    ) -> Result<()> {
        info!("Launching Minecraft {}", version_info.id);

        // Get the appropriate Java installation for this Minecraft version
        let java_installation = java_manager.get_java_for_minecraft(&version_info.id)?;
        info!(
            "Using Java {} at {}",
            java_installation.major_version,
            java_installation.path.display()
        );

        // Determine game directory (instance-specific or default)
        let game_dir = if let Some(inst) = instance {
            info!("Using instance: {}", inst.name);
            minecraft_dir.base_path.join("instances").join(&inst.name)
        } else {
            minecraft_dir.base_path.clone()
        };

        // Build the command
        let mut cmd = Command::new(&java_installation.path);

        // Add JVM arguments (with instance-specific memory settings)
        self.add_jvm_arguments(&mut cmd, version_info, minecraft_dir, instance)?;

        // Add classpath
        self.add_classpath(&mut cmd, version_info, minecraft_dir)?;

        // Add main class
        cmd.arg(&version_info.main_class);

        // Add game arguments (with instance-specific game directory)
        self.add_game_arguments(&mut cmd, version_info, auth, minecraft_dir, instance)?;

        // Set working directory to the game directory
        cmd.current_dir(&game_dir);

        // Configure stdio
        cmd.stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit());

        info!("Starting Minecraft process...");
        info!("Java command: {:?}", cmd);
        if let Some(_inst) = instance {
            info!("Game directory: {}", game_dir.display());
        }

        // Launch the game
        let mut child = cmd.spawn().context("Failed to start Minecraft process")?;

        info!("Minecraft process started with PID: {}", child.id());

        // Wait for the process to complete
        let status = child
            .wait()
            .context("Failed to wait for Minecraft process")?;

        if status.success() {
            info!("Minecraft exited successfully");
        } else {
            return Err(anyhow!("Minecraft exited with code: {:?}", status.code()));
        }

        Ok(())
    }

    /// Add JVM arguments to the command
    fn add_jvm_arguments(
        &self,
        cmd: &mut Command,
        version_info: &VersionInfo,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<()> {
        // Use instance-specific memory settings or defaults
        let (min_mem, max_mem) = if let Some(_inst) = instance {
            if let Some(memory) = _inst.settings.memory_mb {
                let min = format!("-Xms{}M", memory / 2); // Allocate half as minimum
                let max = format!("-Xmx{}M", memory);
                (min, max)
            } else {
                ("-Xms1G".to_string(), "-Xmx2G".to_string())
            }
        } else {
            ("-Xms1G".to_string(), "-Xmx2G".to_string())
        };

        // Memory and GC arguments
        cmd.args([
            &min_mem,
            &max_mem,
            "-XX:+UseG1GC",
            "-XX:+UnlockExperimentalVMOptions",
            "-XX:G1NewSizePercent=20",
            "-XX:G1ReservePercent=20",
            "-XX:MaxGCPauseMillis=50",
            "-XX:G1HeapRegionSize=32M",
        ]);

        // Add custom Java arguments from instance
        if let Some(inst) = instance {
            for arg in &inst.settings.java_args {
                cmd.arg(arg);
            }
        }

        // Add natives library path
        let natives_dir = minecraft_dir.natives_dir(&version_info.id);
        cmd.arg(format!("-Djava.library.path={}", natives_dir.display()));

        // Add Minecraft-specific system properties
        cmd.args([
            "-Dminecraft.launcher.brand=rustified",
            "-Dminecraft.launcher.version=1.0.0",
        ]);

        // Add version-specific JVM arguments if present
        if let Some(arguments) = &version_info.arguments {
            if let Some(jvm_args) = &arguments.jvm {
                for arg in jvm_args {
                    self.add_conditional_jvm_argument(
                        cmd,
                        arg,
                        version_info,
                        minecraft_dir,
                        instance,
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Add classpath to the command
    fn add_classpath(
        &self,
        cmd: &mut Command,
        version_info: &VersionInfo,
        minecraft_dir: &MinecraftDir,
    ) -> Result<()> {
        let mut classpath = Vec::new();

        // Add main game JAR
        let game_jar = minecraft_dir.version_jar_path(&version_info.id);
        if game_jar.exists() {
            debug!(
                "Adding to classpath (main game JAR): {}",
                game_jar.display()
            );
            classpath.push(game_jar.to_string_lossy().to_string());
        } else {
            log::error!("Main game JAR not found: {}", game_jar.display());
            return Err(anyhow!("Main game JAR not found: {}", game_jar.display()));
        }

        // Add library JARs
        for library in &version_info.libraries {
            if !library.should_use() {
                debug!("Skipping library (rules): {}", library.name);
                continue;
            }

            // Check if this library is primarily a native library (e.g., name contains ":natives-")
            // Such libraries should not have their "artifact" (which is the native jar itself) added to the classpath.
            // Their contents are handled by java.library.path.
            let is_explicitly_native_library = library.is_native_library();

            if let Some(_artifact_download) = &library.downloads.artifact {
                if is_explicitly_native_library {
                    debug!(
                        "Skipping classpath addition for explicitly native library artifact: {}",
                        library.name
                    );
                } else {
                    // This is a non-native library with a main artifact. Add it to classpath.
                    // Example: org.lwjgl:lwjgl:3.3.3, org.lwjgl:lwjgl-glfw:3.3.3
                    let relative_lib_path = get_library_path(&library.name); // Use imported function
                    let full_path = minecraft_dir.library_path(&relative_lib_path);

                    if full_path.exists() {
                        debug!("Adding to classpath: {}", full_path.display());
                        classpath.push(full_path.to_string_lossy().to_string());
                    } else {
                        // This could happen if download_libraries failed or json is inconsistent
                        log::warn!(
                            "Library artifact for {} (expected at {}) not found, skipping classpath addition.",
                            library.name,
                            full_path.display()
                        );
                    }
                }
            } else {
                debug!(
                    "Library {} has no main artifact, not adding to classpath directly.",
                    library.name
                );
            }

            // Logging for classified natives (this part is for information, doesn't add to classpath)
            // This confirms that native parts are recognized.
            if library.downloads.classifiers.is_some() {
                if let Some(native_classifier) = library.get_native_classifier() {
                    debug!(
                        "Library {} has native classifier: {}. These are handled by java.library.path.",
                        library.name, native_classifier
                    );
                }
            }
        }

        if classpath.is_empty() {
            log::error!("Classpath is empty! This will likely cause a NoClassDefFoundError.");
            return Err(anyhow!("Classpath construction failed, no entries found."));
        }

        // Join classpath with platform-specific separator
        let separator = if cfg!(windows) { ";" } else { ":" };
        let classpath_str = classpath.join(separator);

        debug!("Final Classpath: {}", classpath_str);
        cmd.args(["-cp", &classpath_str]);

        Ok(())
    }

    /// Add game arguments to the command
    fn add_game_arguments(
        &self,
        cmd: &mut Command,
        version_info: &VersionInfo,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<()> {
        // Handle modern argument format (1.13+)
        if let Some(arguments) = &version_info.arguments {
            if let Some(game_args) = &arguments.game {
                for arg in game_args {
                    self.add_conditional_argument(
                        cmd,
                        arg,
                        version_info,
                        auth,
                        minecraft_dir,
                        instance,
                    )?;
                }
            }
            // Modern versions have comprehensive arguments, so we don't need to add essential arguments
        }
        // Handle legacy argument format (pre-1.13)
        else if let Some(minecraft_arguments) = &version_info.minecraft_arguments {
            let args =
                self.parse_legacy_arguments(minecraft_arguments, auth, minecraft_dir, instance)?;
            cmd.args(args);

            // For legacy versions, add essential arguments that might be missing
            self.add_essential_arguments(cmd, auth, minecraft_dir, instance)?;
        }

        Ok(())
    }

    /// Add conditional argument based on rules
    fn add_conditional_argument(
        &self,
        cmd: &mut Command,
        arg: &ArgumentValue,
        version_info: &VersionInfo,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<()> {
        match arg {
            ArgumentValue::Simple(value) => {
                let resolved = self.resolve_argument_variables(
                    value,
                    version_info,
                    auth,
                    minecraft_dir,
                    instance,
                )?;
                // Filter out demo argument when user has valid auth
                if resolved != "--demo" {
                    cmd.arg(resolved);
                }
            }
            ArgumentValue::Conditional { rules, value } => {
                // Check if rules match current environment
                if self.evaluate_rules(rules)? {
                    match value {
                        ArgumentValueType::Single(val) => {
                            let resolved = self.resolve_argument_variables(
                                val,
                                version_info,
                                auth,
                                minecraft_dir,
                                instance,
                            )?;
                            // Filter out demo argument when user has valid auth
                            if resolved != "--demo" {
                                cmd.arg(resolved);
                            }
                        }
                        ArgumentValueType::Multiple(vals) => {
                            for val in vals {
                                let resolved = self.resolve_argument_variables(
                                    val,
                                    version_info,
                                    auth,
                                    minecraft_dir,
                                    instance,
                                )?;
                                // Filter out demo argument when user has valid auth
                                if resolved != "--demo" {
                                    cmd.arg(resolved);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Add conditional JVM argument based on rules (without auth parameters)
    fn add_conditional_jvm_argument(
        &self,
        cmd: &mut Command,
        arg: &ArgumentValue,
        version_info: &VersionInfo,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<()> {
        match arg {
            ArgumentValue::Simple(value) => {
                let resolved = self.resolve_jvm_argument_variables(
                    value,
                    version_info,
                    minecraft_dir,
                    instance,
                )?;
                cmd.arg(resolved);
            }
            ArgumentValue::Conditional { rules, value } => {
                // Check if rules match current environment
                if self.evaluate_rules(rules)? {
                    match value {
                        ArgumentValueType::Single(val) => {
                            let resolved = self.resolve_jvm_argument_variables(
                                val,
                                version_info,
                                minecraft_dir,
                                instance,
                            )?;
                            cmd.arg(resolved);
                        }
                        ArgumentValueType::Multiple(vals) => {
                            for val in vals {
                                let resolved = self.resolve_jvm_argument_variables(
                                    val,
                                    version_info,
                                    minecraft_dir,
                                    instance,
                                )?;
                                cmd.arg(resolved);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse legacy argument string
    fn parse_legacy_arguments(
        &self,
        arguments: &str,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<Vec<String>> {
        let mut args = Vec::new();

        // Split by spaces but handle quoted strings
        let parts: Vec<&str> = arguments.split_whitespace().collect();

        for part in parts {
            if part.starts_with("${") && part.ends_with("}") {
                // Handle variable substitution
                let var_name = &part[2..part.len() - 1];
                let resolved =
                    self.resolve_legacy_variable(var_name, auth, minecraft_dir, instance)?;
                args.push(resolved);
            } else {
                // Filter out demo argument when user has valid auth
                if part != "--demo" {
                    args.push(part.to_string());
                }
            }
        }

        Ok(args)
    }

    /// Resolve argument variables
    fn resolve_argument_variables(
        &self,
        arg: &str,
        version_info: &VersionInfo,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<String> {
        let mut resolved = arg.to_string();

        // Determine game directory (instance-specific or default)
        let game_dir = if let Some(inst) = instance {
            minecraft_dir.base_path.join("instances").join(&inst.name)
        } else {
            minecraft_dir.base_path.clone()
        };

        // Common variable substitutions
        resolved = resolved.replace("${game_directory}", &game_dir.to_string_lossy());
        resolved = resolved.replace(
            "${assets_root}",
            &minecraft_dir.assets_dir().to_string_lossy(),
        );
        resolved = resolved.replace("${assets_index_name}", &version_info.assets);
        resolved = resolved.replace("${version_name}", &version_info.id);
        resolved = resolved.replace(
            "${version_type}",
            &format!("{:?}", version_info.version_type).to_lowercase(),
        );
        resolved = resolved.replace("${launcher_name}", "rustified");
        resolved = resolved.replace("${launcher_version}", "1.0.0");
        resolved = resolved.replace(
            "${natives_directory}",
            &minecraft_dir
                .natives_dir(&version_info.id)
                .to_string_lossy(),
        );

        // Auth-related variables
        resolved = resolved.replace("${auth_player_name}", &auth.profile.name);
        resolved = resolved.replace("${auth_uuid}", &auth.profile.id);
        resolved = resolved.replace("${auth_access_token}", &auth.access_token);
        resolved = resolved.replace("${user_type}", "msa");

        // Additional variables that may be present in newer versions
        resolved = resolved.replace("${clientid}", ""); // Not needed for our launcher
        resolved = resolved.replace("${auth_xuid}", ""); // Not needed for our launcher
        resolved = resolved.replace("${resolution_width}", "854"); // Default resolution
        resolved = resolved.replace("${resolution_height}", "480"); // Default resolution
        resolved = resolved.replace("${quickPlayPath}", ""); // Empty to avoid Quick Play errors
        resolved = resolved.replace("${quickPlaySingleplayer}", "");
        resolved = resolved.replace("${quickPlayMultiplayer}", "");
        resolved = resolved.replace("${quickPlayRealms}", "");

        Ok(resolved)
    }

    /// Resolve JVM argument variables (without auth information)
    fn resolve_jvm_argument_variables(
        &self,
        arg: &str,
        version_info: &VersionInfo,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<String> {
        let mut resolved = arg.to_string();

        // Determine game directory (instance-specific or default)
        let game_dir = if let Some(inst) = instance {
            minecraft_dir.base_path.join("instances").join(&inst.name)
        } else {
            minecraft_dir.base_path.clone()
        };

        // Common variable substitutions (no auth-related variables for JVM args)
        resolved = resolved.replace("${game_directory}", &game_dir.to_string_lossy());
        resolved = resolved.replace(
            "${assets_root}",
            &minecraft_dir.assets_dir().to_string_lossy(),
        );
        resolved = resolved.replace("${assets_index_name}", &version_info.assets);
        resolved = resolved.replace("${version_name}", &version_info.id);
        resolved = resolved.replace(
            "${version_type}",
            &format!("{:?}", version_info.version_type).to_lowercase(),
        );
        resolved = resolved.replace("${launcher_name}", "rustified");
        resolved = resolved.replace("${launcher_version}", "1.0.0");
        resolved = resolved.replace(
            "${natives_directory}",
            &minecraft_dir
                .natives_dir(&version_info.id)
                .to_string_lossy(),
        );

        Ok(resolved)
    }

    /// Resolve legacy variable names
    fn resolve_legacy_variable(
        &self,
        var_name: &str,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<String> {
        // Determine game directory (instance-specific or default)
        let game_dir = if let Some(inst) = instance {
            minecraft_dir.base_path.join("instances").join(&inst.name)
        } else {
            minecraft_dir.base_path.clone()
        };

        match var_name {
            "auth_player_name" => Ok(auth.profile.name.clone()),
            "auth_uuid" => Ok(auth.profile.id.clone()),
            "auth_access_token" => Ok(auth.access_token.clone()),
            "user_type" => Ok("msa".to_string()),
            "game_directory" => Ok(game_dir.to_string_lossy().to_string()),
            "assets_root" => Ok(minecraft_dir.assets_dir().to_string_lossy().to_string()),
            _ => Err(anyhow!("Unknown legacy variable: {}", var_name)),
        }
    }

    /// Add essential arguments that might be missing
    fn add_essential_arguments(
        &self,
        cmd: &mut Command,
        auth: &AuthResult,
        minecraft_dir: &MinecraftDir,
        instance: Option<&InstanceConfig>,
    ) -> Result<()> {
        // Determine game directory (instance-specific or default)
        let game_dir = if let Some(inst) = instance {
            minecraft_dir.base_path.join("instances").join(&inst.name)
        } else {
            minecraft_dir.base_path.clone()
        };

        // These are essential for modern Minecraft
        cmd.args([
            "--username",
            &auth.profile.name,
            "--uuid",
            &auth.profile.id,
            "--accessToken",
            &auth.access_token,
            "--userType",
            "msa",
            "--gameDir",
            &game_dir.to_string_lossy(),
        ]);

        Ok(())
    }

    /// Evaluate rules for conditional arguments
    fn evaluate_rules(&self, rules: &[crate::launcher::version::Rule]) -> Result<bool> {
        for rule in rules {
            let matches = if let Some(os_rule) = &rule.os {
                if let Some(name) = &os_rule.name {
                    match name.as_str() {
                        "windows" => env::consts::OS == "windows",
                        "linux" => env::consts::OS == "linux",
                        "osx" => env::consts::OS == "macos",
                        _ => false,
                    }
                } else {
                    true
                }
            } else {
                true
            };

            if matches {
                return Ok(rule.action == "allow");
            }
        }
        Ok(false)
    }
}
