use clap::{Parser, Subcommand};
use colored::*;
use chrono::Local;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use gix;
use gix::bstr::ByteSlice;
use serde::{Deserialize, Serialize};

/// Configuration for loco-pilot
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// The default style to use for the prompt
    style: String,
    /// Whether to show git information
    show_git: bool,
    /// Custom colors for different parts of the prompt
    colors: ColorConfig,
}

/// Color configuration
#[derive(Debug, Serialize, Deserialize)]
struct ColorConfig {
    username: String,
    hostname: String,
    directory: String,
    git_branch: String,
    git_dirty: String,
    time: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            style: "default".to_string(),
            show_git: true,
            colors: ColorConfig::default(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        ColorConfig {
            username: "green".to_string(),
            hostname: "yellow".to_string(),
            directory: "cyan".to_string(),
            git_branch: "green".to_string(),
            git_dirty: "red".to_string(),
            time: "blue".to_string(),
        }
    }
}

/// Gets the config file path
fn get_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|mut path| {
        path.push("loco-pilot");
        fs::create_dir_all(&path).ok()?;
        path.push("config.toml");
        Some(path)
    })?
}

/// Load configuration from file
fn load_config() -> Config {
    if let Some(path) = get_config_path() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(config) = toml::from_str::<Config>(&content) {
                return config;
            }
        }
    }
    Config::default()
}

/// Save configuration to file
fn save_config(config: &Config) -> io::Result<()> {
    let config_path = get_config_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine config directory",
        )
    })?;
    
    let content = toml::to_string_pretty(&config)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    
    let mut file = fs::File::create(config_path)?;
    file.write_all(content.as_bytes())?;
    
    Ok(())
}

/// Enable colors even when not in a terminal
fn enable_colors_for_bash() {
    // Force colored to always output colors, even in non-tty environments
    colored::control::set_override(true);
}

/// A customizable bash prompt application
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The style of prompt to display
    #[arg(short, long, default_value = "default")]
    style: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure prompt settings
    Config {
        /// The key to set
        key: Option<String>,
        /// The value to set
        value: Option<String>,
    },
    
    /// Display detailed version information
    Version,
}

/// Returns the current working directory, with home directory replaced by ~
fn get_current_dir() -> String {
    let current_dir = env::current_dir().unwrap_or_default();
    if let Some(home_dir) = dirs::home_dir() {
        let current_path = current_dir.display().to_string();
        let home_path = home_dir.display().to_string();
        
        if current_path.starts_with(&home_path) {
            return current_path.replacen(&home_path, "~", 1);
        }
    }
    
    current_dir.display().to_string()
}

/// Returns a shortened version of the current directory path if it's longer than 15 characters
/// Always keeps the last two path components and abbreviates the middle with "..."
fn get_shortened_dir() -> String {
    let full_path = get_current_dir();
    
    // If the path is short enough, return it as is
    if full_path.len() <= 15 {
        return full_path;
    }
    
    // Split the path by separator
    let components: Vec<&str> = full_path.split('/').collect();
    
    // If we have 3 or fewer components, just return the full path
    if components.len() <= 3 {
        return full_path;
    }
    
    // Get the first component (usually ~ or root)
    let first = components.first().unwrap_or(&"");
    
    // Get the last two components
    let last_two = if components.len() >= 2 {
        let len = components.len();
        format!("{}/{}", components[len - 2], components[len - 1])
    } else {
        components.last().unwrap_or(&"").to_string()
    };
    
    // Format with ellipsis
    format!("{}/.../{}", first, last_two)
}

/// Get the hostname of the machine
fn get_hostname() -> String {
    // Try multiple ways to get the hostname
    if let Ok(hostname) = env::var("HOSTNAME") {
        return hostname;
    }
    
    if let Ok(hostname) = env::var("HOST") {
        return hostname;
    }
    
    // Try to get hostname using the hostname command
    if let Ok(output) = Command::new("hostname").output() {
        if let Ok(hostname) = String::from_utf8(output.stdout) {
            return hostname.trim().to_string();
        }
    }
    
    // Fallback
    "localhost".to_string()
}

/// Git repository status information
#[derive(Debug)]
struct GitStatus {
    branch: String,
    dirty: bool,
    #[allow(dead_code)]
    ahead: usize,
    #[allow(dead_code)]
    behind: usize,
}

/// Get git branch information if in a git repository
fn get_git_info() -> Option<GitStatus> {
    // Try to open the git repository at the current directory
    let current_dir = env::current_dir().ok()?;
    match gix::open(&current_dir) {
        Ok(repo) => {
            // Get the current branch name
            let branch = if let Ok(head) = repo.head() {
                let reference_str = head.name().as_bstr().to_str().unwrap_or("HEAD");
                
                // Use external git command to get branch name, which is more reliable
                if let Ok(output) = Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .current_dir(&current_dir)
                    .output() 
                {
                    if output.status.success() {
                        if let Ok(branch_name) = String::from_utf8(output.stdout) {
                            let branch_name = branch_name.trim();
                            if branch_name == "HEAD" {
                                // We're in detached HEAD state, get the commit hash
                                if let Ok(commit_output) = Command::new("git")
                                    .args(["rev-parse", "--short", "HEAD"])
                                    .current_dir(&current_dir)
                                    .output() 
                                {
                                    if let Ok(commit_hash) = String::from_utf8(commit_output.stdout) {
                                        format!("detached@{}", commit_hash.trim())
                                    } else {
                                        "detached HEAD".to_string()
                                    }
                                } else {
                                    "detached HEAD".to_string()
                                }
                            } else {
                                branch_name.to_string()
                            }
                        } else {
                            // Fallback to reference name if command output isn't valid UTF-8
                            if reference_str.starts_with("refs/heads/") {
                                reference_str.trim_start_matches("refs/heads/").to_string()
                            } else {
                                reference_str.to_string()
                            }
                        }
                    } else {
                        // Fallback to reference name if command fails
                        if reference_str.starts_with("refs/heads/") {
                            reference_str.trim_start_matches("refs/heads/").to_string()
                        } else {
                            reference_str.to_string()
                        }
                    }
                } else {
                    // Fallback to reference name if command execution fails
                    if reference_str.starts_with("refs/heads/") {
                        reference_str.trim_start_matches("refs/heads/").to_string()
                    } else {
                        reference_str.to_string()
                    }
                }
            } else {
                "unknown".to_string()
            };
            
            // Check for uncommitted changes
            let mut dirty = false;
            
            // First check if there's any in-progress operation
            if repo.state().is_some() {
                dirty = true;
            } else {
                // Check for uncommitted changes using git status
                if let Ok(status) = Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(&current_dir)
                    .output() 
                {
                    if !status.stdout.is_empty() {
                        dirty = true;
                    }
                }
            }
            
            // Check for ahead/behind count using git rev-list
            let mut ahead = 0;
            let mut behind = 0;
            
            if !branch.starts_with("detached") {
                if let Ok(output) = Command::new("git")
                    .args(["rev-list", "--left-right", "--count", "@{u}...HEAD"])
                    .current_dir(&current_dir)
                    .output()
                {
                    // Safely handle the output - only proceed if we have a valid output
                    if output.status.success() {
                        if let Ok(counts) = String::from_utf8(output.stdout) {
                            let counts: Vec<&str> = counts.trim().split_whitespace().collect();
                            if counts.len() == 2 {
                                behind = counts[0].parse().unwrap_or(0);
                                ahead = counts[1].parse().unwrap_or(0);
                            }
                        }
                    }
                }
            }
            
            Some(GitStatus {
                branch,
                dirty,
                ahead,
                behind,
            })
        }
        Err(_) => None, // Not in a git repository
    }
}

/// Get the current git commit SHA
fn get_git_commit_sha() -> Option<String> {
    let current_dir = env::current_dir().ok()?;
    
    // Try to get short commit hash using git command
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&current_dir)
        .output() 
    {
        if output.status.success() {
            if let Ok(sha) = String::from_utf8(output.stdout) {
                return Some(sha.trim().to_string());
            }
        }
    }
    
    // Fallback to gix if git command fails
    match gix::open(&current_dir) {
        Ok(repo) => {
            if let Ok(head) = repo.head() {
                // Different approach to get the commit id from gix
                if let Some(id) = head.id() {
                    // Get short SHA (7 characters)
                    let short_id = id.to_string()[..7].to_string();
                    return Some(short_id);
                }
            }
            None
        },
        Err(_) => None,
    }
}

/// Get the full version string including git commit SHA
fn get_full_version() -> String {
    // Get the crate version from Cargo.toml via env
    let version = env!("CARGO_PKG_VERSION");
    
    // Append the git SHA if available
    if let Some(sha) = get_git_commit_sha() {
        format!("{} ({})", version, sha)
    } else {
        version.to_string()
    }
}

/// Generate the prompt string
fn generate_prompt(style: &str) -> String {
    enable_colors_for_bash();
    
    let current_time = Local::now().format("%H:%M:%S").to_string();
    let username = env::var("USER").unwrap_or_else(|_| "user".to_string());
    let hostname = get_hostname();
    let current_dir = get_current_dir();  // Using the full path with ~ instead of shortened path
    
    // Add bash-specific escape sequence markers to prevent prompt length miscalculation
    // \001 and \002 are the actual control characters that Bash uses (octal 001 and 002)
    let wrap_color = |s: String| -> String {
        format!("\x01{}\x02", s)
    };
    
    let git_info = get_git_info()
        .map(|status| {
            let branch_info = match style {
                "emoji" => format!(" 🔖 {}", status.branch),
                _ => {
                    let colored_branch = status.branch.green().to_string();
                    format!(" ({})", wrap_color(colored_branch))
                }
            };
            
            // Add ahead/behind indicators
            let mut ahead_behind = String::new();
            if status.ahead > 0 {
                ahead_behind.push_str(&match style {
                    "emoji" => format!(" ↑{}", status.ahead),
                    _ => format!(" ↑{}", status.ahead)
                });
            }
            if status.behind > 0 {
                ahead_behind.push_str(&match style {
                    "emoji" => format!(" ↓{}", status.behind),
                    _ => format!(" ↓{}", status.behind)
                });
            }
            
            let dirty_info = if status.dirty { 
                match style {
                    "emoji" => " 🔴".to_string(),
                    _ => {
                        let colored_star = " *".red().to_string();
                        wrap_color(colored_star)
                    }
                }
            } else { 
                "".to_string() 
            };
            
            format!("{}{}{}", branch_info, ahead_behind, dirty_info)
        })
        .unwrap_or_default();
    
    match style {
        "minimal" => format!("$ "),
        "info" => format!(
            "[{}] {}@{}: {}{} $ ",
            wrap_color(current_time.blue().to_string()),
            wrap_color(username.green().to_string()),
            wrap_color(hostname.yellow().to_string()),
            wrap_color(current_dir.cyan().to_string()),
            git_info
        ),
        "emoji" => format!(
            "🕒 {} 👤 {} 🖥️  {} 📁 {}{} ➡️  ",
            current_time,
            username,
            hostname,
            current_dir,
            git_info
        ),
        _ => format!(
            "{}@{}:{}{} $ ",
            wrap_color(username.green().to_string()),
            wrap_color(hostname.yellow().to_string()),
            wrap_color(current_dir.cyan().to_string()),
            git_info
        ),
    }
}

fn main() {
    let args = Args::parse();
    // Load configuration
    let mut config = load_config();
    
    match &args.command {
        Some(Commands::Config { key, value }) => {
            // Handle configuration changes
            if let (Some(key), Some(value)) = (key, value) {
                match key.as_str() {
                    "style" => {
                        config.style = value.clone();
                        println!("Default style set to: {}", value);
                    }
                    "show_git" => {
                        config.show_git = value.to_lowercase() == "true";
                        println!("Show git info: {}", config.show_git);
                    }
                    "color.username" => {
                        config.colors.username = value.clone();
                        println!("Username color set to: {}", value);
                    }
                    "color.hostname" => {
                        config.colors.hostname = value.clone();
                        println!("Hostname color set to: {}", value);
                    }
                    "color.directory" => {
                        config.colors.directory = value.clone();
                        println!("Directory color set to: {}", value);
                    }
                    "color.git_branch" => {
                        config.colors.git_branch = value.clone();
                        println!("Git branch color set to: {}", value);
                    }
                    "color.git_dirty" => {
                        config.colors.git_dirty = value.clone();
                        println!("Git dirty indicator color set to: {}", value);
                    }
                    "color.time" => {
                        config.colors.time = value.clone();
                        println!("Time color set to: {}", value);
                    }
                    _ => {
                        println!("Unknown configuration key: {}", key);
                        return;
                    }
                }
                
                // Save updated configuration
                if let Err(e) = save_config(&config) {
                    eprintln!("Failed to save configuration: {}", e);
                } else {
                    println!("Configuration saved successfully");
                }
            } else {
                // If no key/value provided, show current configuration
                println!("Current configuration:");
                println!("  style = {}", config.style);
                println!("  show_git = {}", config.show_git);
                println!("  color.username = {}", config.colors.username);
                println!("  color.hostname = {}", config.colors.hostname);
                println!("  color.directory = {}", config.colors.directory);
                println!("  color.git_branch = {}", config.colors.git_branch);
                println!("  color.git_dirty = {}", config.colors.git_dirty);
                println!("  color.time = {}", config.colors.time);
            }
        }
        Some(Commands::Version) => {
            println!("Version: {}", get_full_version());
        }
        None => {
            // Use style from command line args if provided, otherwise use from config
            let style = if args.style != "default" {
                args.style
            } else {
                config.style
            };
            
            // Generate and print the prompt
            print!("{}", generate_prompt(&style));
        }
    }
}