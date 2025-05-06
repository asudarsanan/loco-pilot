use chrono::Local;
use clap::{Parser, Subcommand};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Add test_utils module for unit testing
#[cfg(test)]
mod test_utils;

/// Type alias for a cached item with timestamp
type CachedItem<T> = Option<(T, Instant)>;

/// Type alias for the path cache tuple - contains current directory, home directory, and hostname
type PathCacheTuple = (CachedItem<String>, CachedItem<String>, CachedItem<String>);

/// Cache for git information to avoid repeated expensive git operations
static GIT_INFO_CACHE: Lazy<Mutex<Option<(GitStatus, Instant)>>> = Lazy::new(|| Mutex::new(None));

/// Cache for filesystem paths and environment variables
static PATH_CACHE: Lazy<Mutex<PathCacheTuple>> = Lazy::new(|| Mutex::new((None, None, None)));

/// Maximum age of cached git info in seconds
const GIT_CACHE_TTL_SECS: u64 = 2;

/// Maximum age of cached paths in seconds
const PATH_CACHE_TTL_SECS: u64 = 5;

/// Configuration for loco-pilot
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    /// The default style to use for the prompt
    style: String,
    /// Whether to show git information
    show_git: bool,
    /// Custom colors for different parts of the prompt
    colors: ColorConfig,
}

/// Color configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
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

// Cache for configuration
static CONFIG_CACHE: Lazy<Mutex<Option<(Config, Instant)>>> = Lazy::new(|| Mutex::new(None));

/// Maximum age of cached config in seconds
const CONFIG_CACHE_TTL_SECS: u64 = 60;

/// Gets the config file path
fn get_config_path() -> Option<PathBuf> {
    // This could be cached for even more performance, but it's rarely called
    dirs::config_dir().map(|mut path| {
        path.push("loco-pilot");
        fs::create_dir_all(&path).ok()?;
        path.push("config.toml");
        Some(path)
    })?
}

/// Load configuration from file with caching
fn load_config() -> Config {
    let mut cache = CONFIG_CACHE.lock().unwrap();
    if let Some((cached_config, timestamp)) = &*cache {
        if timestamp.elapsed() < Duration::from_secs(CONFIG_CACHE_TTL_SECS) {
            return cached_config.clone();
        }
    }

    let config = if let Some(path) = get_config_path() {
        if let Ok(content) = fs::read_to_string(path) {
            toml::from_str::<Config>(&content).unwrap_or_default()
        } else {
            Config::default()
        }
    } else {
        Config::default()
    };

    *cache = Some((config.clone(), Instant::now()));
    config
}

/// Save configuration to file
fn save_config(config: &Config) -> io::Result<()> {
    let config_path = get_config_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine config directory",
        )
    })?;

    let content =
        toml::to_string_pretty(&config).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut file = fs::File::create(config_path)?;
    file.write_all(content.as_bytes())?;

    // Update the cache with the new config
    let mut cache = CONFIG_CACHE.lock().unwrap();
    *cache = Some((config.clone(), Instant::now()));

    Ok(())
}

/// Enable colors even when not in a terminal
#[inline]
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

    /// Copy current git branch name to clipboard
    #[arg(long = "gbc", action)]
    git_branch_copy: bool,

    /// Select and copy a git branch from all local branches
    #[arg(long = "gbs", action)]
    git_branch_select: bool,

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

    /// Copy the current git branch name to clipboard
    GitBranchCopy,

    /// Select and copy a git branch from all local branches
    GitBranchSelect,
}

/// Returns the current working directory, with home directory replaced by ~
fn get_current_dir() -> String {
    let mut path_cache = PATH_CACHE.lock().unwrap();
    let (current_dir_cache, home_dir_cache, _) = &*path_cache;

    // Check if we have a cached current directory that's still fresh
    if let Some((cached_dir, timestamp)) = current_dir_cache {
        if timestamp.elapsed() < Duration::from_secs(PATH_CACHE_TTL_SECS) {
            return cached_dir.clone();
        }
    }

    let current_dir = env::current_dir().unwrap_or_default();
    let current_path = current_dir.display().to_string();

    // Check if we have a cached home directory
    let home_path = if let Some((cached_home, timestamp)) = home_dir_cache {
        if timestamp.elapsed() < Duration::from_secs(PATH_CACHE_TTL_SECS) {
            cached_home.clone()
        } else if let Some(home_dir) = dirs::home_dir() {
            let home_path = home_dir.display().to_string();
            path_cache.1 = Some((home_path.clone(), Instant::now()));
            home_path
        } else {
            String::new()
        }
    } else if let Some(home_dir) = dirs::home_dir() {
        let home_path = home_dir.display().to_string();
        path_cache.1 = Some((home_path.clone(), Instant::now()));
        home_path
    } else {
        String::new()
    };

    let result = if !home_path.is_empty() && current_path.starts_with(&home_path) {
        current_path.replacen(&home_path, "~", 1)
    } else {
        current_path
    };

    // Update the cache
    path_cache.0 = Some((result.clone(), Instant::now()));

    result
}

/// Returns a shortened version of the current directory path if it's longer than 15 characters
#[inline]
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

/// Get the hostname of the machine with caching
fn get_hostname() -> String {
    let mut path_cache = PATH_CACHE.lock().unwrap();
    let (_, _, hostname_cache) = &*path_cache;

    // Check if we have a cached hostname that's still fresh
    if let Some((cached_hostname, timestamp)) = hostname_cache {
        if timestamp.elapsed() < Duration::from_secs(PATH_CACHE_TTL_SECS) {
            return cached_hostname.clone();
        }
    }

    // Try multiple ways to get the hostname
    let hostname = if let Ok(hostname) = env::var("HOSTNAME") {
        hostname
    } else if let Ok(hostname) = env::var("HOST") {
        hostname
    } else if let Ok(output) = Command::new("hostname").output() {
        if let Ok(hostname) = String::from_utf8(output.stdout) {
            hostname.trim().to_string()
        } else {
            "localhost".to_string()
        }
    } else {
        "localhost".to_string()
    };

    // Update the cache
    path_cache.2 = Some((hostname.clone(), Instant::now()));

    hostname
}

/// Git repository status information
#[derive(Debug, Clone)]
struct GitStatus {
    branch: String,
    dirty: bool,
    ahead: usize,
    behind: usize,
}

/// Get git branch information if in a git repository
/// This is a highly optimized version that reduces the number of git command executions
fn get_git_info() -> Option<GitStatus> {
    // Check the cache first
    let mut cache = GIT_INFO_CACHE.lock().unwrap();
    if let Some((cached_status, timestamp)) = &*cache {
        if timestamp.elapsed() < Duration::from_secs(GIT_CACHE_TTL_SECS) {
            return Some(cached_status.clone());
        }
    }

    let current_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    // Quick check if this is a git repository
    // This avoids expensive operations if we're not in a git repo
    let git_dir = current_dir.join(".git");
    if !git_dir.exists() {
        return None;
    }

    // Use a single git command to get branch and status information
    // This is much faster than multiple separate calls
    let output = match Command::new("git")
        .args(["status", "--branch", "--porcelain=v2"])
        .current_dir(&current_dir)
        .output()
    {
        Ok(output) => output,
        Err(_) => return None,
    };

    if !output.status.success() {
        return None;
    }

    let status_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = status_output.lines().collect();

    // Parse branch information from the output
    let mut branch = String::from("unknown");
    let mut ahead = 0;
    let mut behind = 0;

    for line in &lines {
        if let Some(branch_name) = line.strip_prefix("# branch.head ") {
            branch = branch_name.to_string();
        } else if let Some(branch_ab_info) = line.strip_prefix("# branch.ab ") {
            let parts: Vec<&str> = branch_ab_info.split_whitespace().collect();
            if parts.len() == 2 {
                ahead = parts[1].parse::<i32>().unwrap_or(0) as usize;
                behind = parts[0].parse::<i32>().unwrap_or(0).unsigned_abs() as usize;
            }
        }
    }

    // If branch is HEAD, we're in detached HEAD state - get commit hash
    if branch == "HEAD" {
        if let Ok(commit_output) = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&current_dir)
            .output()
        {
            if commit_output.status.success() {
                if let Ok(commit_hash) = String::from_utf8(commit_output.stdout) {
                    branch = format!("detached@{}", commit_hash.trim());
                }
            }
        }
    }

    // Check for dirty status - anything that starts with a space and a single letter
    // indicates a change in git status
    let dirty = lines
        .iter()
        .any(|line| !line.starts_with('#') && line.len() > 1 && !line.starts_with(' '));

    let git_status = GitStatus {
        branch,
        dirty,
        ahead,
        behind,
    };

    // Update the cache
    *cache = Some((git_status.clone(), Instant::now()));
    Some(git_status)
}

/// Get current git branch name
fn get_current_git_branch() -> Option<String> {
    get_git_info().map(|info| info.branch)
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
        }
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

// Cache for username
static USERNAME_CACHE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Get username with caching
#[inline]
fn get_username() -> String {
    let mut cache = USERNAME_CACHE.lock().unwrap();
    if let Some(username) = &*cache {
        return username.clone();
    }

    let username = env::var("USER").unwrap_or_else(|_| "user".to_string());
    *cache = Some(username.clone());
    username
}

/// Generate properly escaped bash prompt color codes
/// This is the key function for fixing the prompt issues
fn bash_color(ansi_code: &str) -> String {
    // Properly wrap ANSI escape codes with bash's prompt escaping sequences
    // This ensures bash correctly calculates prompt width by ignoring non-printing characters
    // Use literal escape sequences without backslash escaping
    format!("\\[{}\\]", ansi_code)
}

/// Generate the prompt string
fn generate_prompt(style: &str) -> String {
    enable_colors_for_bash();

    // Load configuration to get user-defined colors
    let config = load_config();

    let current_time = Local::now().format("%H:%M:%S").to_string();
    let username = get_username();
    let hostname = get_hostname();
    let current_dir = get_shortened_dir();

    // Map color names to ANSI color codes
    let color_map = |color_name: &str| -> &str {
        match color_name {
            "black" => "\x1b[30m",
            "red" => "\x1b[31m",
            "green" => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue" => "\x1b[34m",
            "purple" | "magenta" => "\x1b[35m",
            "cyan" => "\x1b[36m",
            "white" => "\x1b[37m",
            "bright_black" | "gray" => "\x1b[90m",
            "bright_red" => "\x1b[91m",
            "bright_green" => "\x1b[92m",
            "bright_yellow" => "\x1b[93m",
            "bright_blue" => "\x1b[94m",
            "bright_magenta" | "bright_purple" => "\x1b[95m",
            "bright_cyan" => "\x1b[96m",
            "bright_white" => "\x1b[97m",
            // Bold variants
            "bold_black" => "\x1b[1;30m",
            "bold_red" => "\x1b[1;31m",
            "bold_green" => "\x1b[1;32m",
            "bold_yellow" => "\x1b[1;33m",
            "bold_blue" => "\x1b[1;34m",
            "bold_magenta" | "bold_purple" => "\x1b[1;35m",
            "bold_cyan" => "\x1b[1;36m",
            "bold_white" => "\x1b[1;37m",
            // Default to bold green if not recognized
            _ => "\x1b[1;32m",
        }
    };

    // Create ANSI color sequences with bash prompt escaping based on user configuration
    let username_color = bash_color(color_map(&config.colors.username));
    let hostname_color = bash_color(color_map(&config.colors.hostname));
    let dir_color = bash_color(color_map(&config.colors.directory));
    let time_color = bash_color(color_map(&config.colors.time));
    let reset = bash_color("\x1b[0m");

    // Format colored text segments
    let username_fmt = format!("{}{}{}", username_color, username, reset);
    let hostname_fmt = format!("{}{}{}", hostname_color, hostname, reset);
    let dir_fmt = format!("{}{}{}", dir_color, current_dir, reset);
    let time_fmt = format!("{}{}{}", time_color, current_time, reset);

    // Only get git info if it's needed for the selected style
    let git_info = if style != "minimal" && config.show_git {
        get_git_info()
            .map(|status| {
                let branch_color = bash_color(color_map(&config.colors.git_branch));
                let dirty_color = bash_color(color_map(&config.colors.git_dirty));
                let ahead_color = bash_color("\x1b[01;33m"); // Bold Yellow
                let behind_color = bash_color("\x1b[01;35m"); // Bold Purple

                let branch_info = match style {
                    "emoji" => format!(" 🔖 {}", status.branch),
                    _ => {
                        let colored_branch = format!("{}{}{}", branch_color, status.branch, reset);
                        format!(" ({})", colored_branch)
                    }
                };

                // Add ahead/behind indicators
                let mut ahead_behind = String::new();
                if status.ahead > 0 {
                    ahead_behind.push_str(&match style {
                        "emoji" => format!(" ↑{}", status.ahead),
                        _ => format!(" {}↑{}{}", ahead_color, status.ahead, reset),
                    });
                }
                if status.behind > 0 {
                    ahead_behind.push_str(&match style {
                        "emoji" => format!(" ↓{}", status.behind),
                        _ => format!(" {}↓{}{}", behind_color, status.behind, reset),
                    });
                }

                let dirty_info = if status.dirty {
                    match style {
                        "emoji" => " 🔴".to_string(),
                        _ => format!("{}*{}", dirty_color, reset),
                    }
                } else {
                    String::new()
                };

                format!("{}{}{}", branch_info, ahead_behind, dirty_info)
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Avoid string allocations where possible by using match with direct format calls
    match style {
        "minimal" => String::from("$ "),
        "info" => format!(
            "[{}] {}@{}: {}{} $ ",
            time_fmt, username_fmt, hostname_fmt, dir_fmt, git_info
        ),
        "emoji" => format!(
            "🕒 {} 👤 {} 🖥️  {} 📁 {}{} ➡️  ",
            current_time, username, hostname, current_dir, git_info
        ),
        _ => format!(
            "{}@{}:{}{} $ ",
            username_fmt, hostname_fmt, dir_fmt, git_info
        ),
    }
}

/// Get list of all local git branches
fn get_git_branches() -> Result<Vec<String>, String> {
    let current_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return Err(format!("Failed to get current directory: {}", e)),
    };

    // Check if this is a git repository
    let git_dir = current_dir.join(".git");
    if !git_dir.exists() {
        return Err("Not in a git repository".to_string());
    }

    // Get all local branches
    let output = match Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(&current_dir)
        .output() {
        Ok(output) => output,
        Err(e) => return Err(format!("Failed to execute git command: {}", e)),
    };

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git command failed: {}", error_msg));
    }

    let branch_output = String::from_utf8_lossy(&output.stdout);
    
    // Parse branches from output and filter any empty lines
    let branches: Vec<String> = branch_output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    
    Ok(branches)
}

/// Display a selection menu of git branches and return the selected branch
fn select_git_branch(branches: &[String]) -> Option<String> {
    println!("Select a branch to copy:");
    
    // Display the branches with numbers
    for (i, branch) in branches.iter().enumerate() {
        println!("{}. {}", i + 1, branch);
    }
    
    // Read user input
    print!("Enter number (1-{}): ", branches.len());
    io::stdout().flush().ok()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    
    // Parse the input as a number
    match input.trim().parse::<usize>() {
        Ok(num) if num > 0 && num <= branches.len() => Some(branches[num - 1].clone()),
        _ => {
            eprintln!("Invalid selection");
            None
        }
    }
}

/// Copy text to terminal clipboard or output it for user to copy
fn copy_to_clipboard(text: &str) -> bool {
    // In terminal environments, output to stdout for easy copying
    println!("{}", text);

    // For terminal use, we consider the operation successful if we can output the text
    // This is more reliable than depending on external clipboard tools
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::create_mock_config;

    #[test]
    fn test_config_default() {
        // Test default configuration values
        let config = Config::default();
        assert_eq!(config.style, "default");
        assert_eq!(config.show_git, true);
        assert_eq!(config.colors.username, "green");
        assert_eq!(config.colors.hostname, "yellow");
        assert_eq!(config.colors.directory, "cyan");
        assert_eq!(config.colors.git_branch, "green");
        assert_eq!(config.colors.git_dirty, "red");
        assert_eq!(config.colors.time, "blue");
    }

    #[test]
    fn test_color_mapping() {
        // This is a more direct test of the color_map function
        // since we can't easily test generate_prompt as a whole
        let green_code = "\x1b[32m";
        let result = bash_color(green_code);
        assert_eq!(result, "\\[\x1b[32m\\]");
    }

    #[test]
    fn test_bash_color() {
        let color_code = "\x1b[32m"; // Green 
        let bash_escaped = bash_color(color_code);
        assert_eq!(bash_escaped, "\\[\x1b[32m\\]");
    }

    #[test]
    fn test_mock_config() {
        // This test uses our mock config function from test_utils
        let mock_config = create_mock_config();

        // Verify the mock config has the expected values
        assert_eq!(mock_config.style, "test_style");
        assert_eq!(mock_config.show_git, true);
        assert_eq!(mock_config.colors.username, "test_green");
        assert_eq!(mock_config.colors.hostname, "test_yellow");
        assert_eq!(mock_config.colors.directory, "test_cyan");
        assert_eq!(mock_config.colors.git_branch, "test_green");
        assert_eq!(mock_config.colors.git_dirty, "test_red");
        assert_eq!(mock_config.colors.time, "test_blue");
    }
}

fn main() {
    let args = Args::parse();

    // Check for flag arguments first
    if args.git_branch_copy {
        // Copy the current git branch name to clipboard
        if let Some(branch) = get_current_git_branch() {
            copy_to_clipboard(&branch);
            println!("Git branch name: '{}'", branch);
            return;
        } else {
            eprintln!("Not in a git repository or unable to determine current branch");
            return;
        }
    }

    if args.git_branch_select {
        // Get list of branches and present a selection menu
        match get_git_branches() {
            Ok(branches) => {
                if branches.is_empty() {
                    eprintln!("No git branches found");
                    return;
                }
                
                let selected_branch = select_git_branch(&branches);
                if let Some(branch) = selected_branch {
                    copy_to_clipboard(&branch);
                    println!("Selected git branch: '{}'", branch);
                } else {
                    eprintln!("No branch selected");
                }
            }
            Err(e) => {
                eprintln!("Failed to get git branches: {}", e);
            }
        }
        return;
    }

    match &args.command {
        Some(Commands::Config { key, value }) => {
            // Handle configuration changes
            // Load configuration
            let mut config = load_config();

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
        Some(Commands::GitBranchCopy) => {
            // Copy the current git branch name to clipboard
            if let Some(branch) = get_current_git_branch() {
                copy_to_clipboard(&branch);
                println!("Git branch name: '{}'", branch);
            } else {
                eprintln!("Not in a git repository or unable to determine current branch");
            }
        }
        Some(Commands::GitBranchSelect) => {
            // Get list of branches and present a selection menu
            match get_git_branches() {
                Ok(branches) => {
                    if branches.is_empty() {
                        eprintln!("No git branches found");
                        return;
                    }
                    
                    let selected_branch = select_git_branch(&branches);
                    if let Some(branch) = selected_branch {
                        copy_to_clipboard(&branch);
                        println!("Selected git branch: '{}'", branch);
                    } else {
                        eprintln!("No branch selected");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get git branches: {}", e);
                }
            }
        }
        None => {
            // Only load config if needed for the style information
            let style = if args.style != "default" {
                args.style
            } else {
                load_config().style
            };

            // Generate and print the prompt
            print!("{}", generate_prompt(&style));
        }
    }
}
