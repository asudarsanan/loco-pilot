// Test utilities for unit tests
#[cfg(test)]
pub mod tests {
    use super::super::*;
    
    /// Creates a mock Config for testing
    pub fn create_mock_config() -> Config {
        Config {
            style: "test_style".to_string(),
            show_git: true,
            colors: ColorConfig {
                username: "test_green".to_string(),
                hostname: "test_yellow".to_string(),
                directory: "test_cyan".to_string(),
                git_branch: "test_green".to_string(),
                git_dirty: "test_red".to_string(),
                time: "test_blue".to_string(),
            },
        }
    }
}