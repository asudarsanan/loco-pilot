# loco-pilot

A customizable bash prompt tool that provides a dynamic, informative command-line prompt with Git integration.

## Features

- Multiple prompt styles (default, minimal, info, emoji)
- Git repository status information (branch, dirty status, ahead/behind count)
- Customizable colors for different prompt components
- Configuration system with persistent settings
- Command-line options to override defaults

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.70 or newer)
- Git (for repository status features)

### Building from Source

1. Clone this repository:
   ```bash
   git clone https://github.com/yourusername/loco-pilot.git
   cd loco-pilot
   ```

2. Build with cargo:
   ```bash
   cargo build --release
   ```

3. The compiled binary will be available at `target/release/loco-pilot`

4. Make it available system-wide (optional):
   ```bash
   sudo cp target/release/loco-pilot /usr/local/bin/
   ```

## Integration with Bash

To use loco-pilot as your bash prompt, you need to add a function to your `~/.bashrc` file:

1. Open your bashrc file:
   ```bash
   nano ~/.bashrc
   ```

2. Add the following function at the end:
   ```bash
   # loco-pilot bash prompt integration
   function set_prompt_command() {
     # Capture the exit code of the last command
     local EXIT_CODE=$?
     # Use loco-pilot to generate the prompt
     PS1=$(loco-pilot)
     # Return the original exit code
     return $EXIT_CODE
   }
   PROMPT_COMMAND=set_prompt_command
   ```

   Alternatively, for a more advanced setup with exit code display:
   ```bash
   # loco-pilot bash prompt integration with exit code display
   function set_prompt_command() {
     local EXIT_CODE=$?
     if [ $EXIT_CODE -ne 0 ]; then
       # Show exit code in red if non-zero
       PS1=$(loco-pilot)"\[\e[31m\][$EXIT_CODE]\[\e[0m\] "
     else
       PS1=$(loco-pilot)
     fi
     return $EXIT_CODE
   }
   PROMPT_COMMAND=set_prompt_command
   ```

3. Save and reload your bashrc:
   ```bash
   source ~/.bashrc
   ```

### Alternative Integration Methods

**Using PROMPT_COMMAND directly:**
```bash
PROMPT_COMMAND='PS1=$(loco-pilot)'
```

**For performance optimization (precompiled binaries):**
```bash
# Use precompiled binary for better performance
if [ -x "$(command -v loco-pilot)" ]; then
  function set_prompt_command() {
    local EXIT_CODE=$?
    PS1=$(loco-pilot)
    return $EXIT_CODE
  }
  PROMPT_COMMAND=set_prompt_command
else
  # Fallback to standard prompt if loco-pilot is not installed
  PS1='\u@\h:\w\$ '
fi
```

## Configuration

loco-pilot provides a configuration system to customize your prompt.

### Command-line Options

- Set a temporary prompt style: `loco-pilot --style emoji`

### Permanent Configuration

Configure settings that persist across sessions:

```bash
# Set default prompt style
loco-pilot config style minimal

# Enable/disable git information
loco-pilot config show_git true

# Customize colors
loco-pilot config color.username blue
loco-pilot config color.hostname yellow
loco-pilot config color.directory cyan
loco-pilot config color.git_branch green
loco-pilot config color.git_dirty red
loco-pilot config color.time blue
```

### View Current Configuration

```bash
loco-pilot config
```

## Available Prompt Styles

### Default
Shows username, hostname, current directory, and git status:
```
username@hostname:~/current/directory (main) $ 
```

### Minimal
A simple prompt with just a dollar sign:
```
$ 
```

### Info
Detailed prompt with timestamp:
```
[12:34:56] username@hostname: ~/current/directory (main) $ 
```

### Emoji
Fun prompt style with emoji icons:
```
🕒 12:34:56 👤 username 🖥️ hostname 📁 ~/current/directory 🔖 main ➡️
```

## Path Shortening

All prompt styles automatically shorten the current working directory path when it's longer than 15 characters:

- The first path component and last two path components are always preserved
- Everything in between is replaced with "..."  
- For example: `/home/user/deeply/nested/folders/project/src` becomes `/home/.../project/src`
- Paths that are 15 characters or shorter remain unchanged
- Home directory is always replaced with `~`

## Continuous Integration and Releases

This project uses GitHub Actions for continuous integration and automatic release management.

### CI Workflow

The CI workflow performs the following checks on each push and pull request:
- Runs cargo check to validate the code
- Compiles the code in debug and release modes
- Runs tests with cargo test
- Performs linting with clippy
- Checks code formatting with rustfmt

### Release Workflow

When a new tag is pushed with a version number (e.g., v0.1.0), the release workflow:
1. Creates a GitHub release
2. Builds binary artifacts for multiple platforms (Linux, macOS, Windows)
3. Attaches the compiled binaries to the release

#### Creating a New Release

To create a new release, follow these steps:

1. Ensure your code is ready for release (all tests pass, features are complete)
2. Update the version in `Cargo.toml` to match the release version
3. Commit the version change:
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to X.Y.Z"
   ```
4. Tag the current commit with a version:
   ```bash
   git tag -a vX.Y.Z -m "Version X.Y.Z"
   ```
   For example: `git tag -a v0.1.0 -m "Version 0.1.0"`
5. Push the commits and the tag to GitHub:
   ```bash
   git push origin main
   git push origin vX.Y.Z
   ```
   For example: `git push origin v0.1.0`
6. The GitHub Actions workflow will automatically build the binaries and create a release

Once the workflow completes, you'll find the release with attached binaries on the GitHub releases page for the repository.

## Troubleshooting

If you encounter issues with git repository detection, ensure:
1. You have git installed on your system
2. The current directory is within a git repository
3. You have appropriate permissions to access the repository

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
