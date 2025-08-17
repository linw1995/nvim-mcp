#[cfg(test)]
mod tests {
    use crate::config::*;
    use tempfile::TempDir;

    #[test]
    fn test_server_config_with_provided_path() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir
            .path()
            .join("custom_socket")
            .to_string_lossy()
            .to_string();

        let config =
            ServerConfig::new(Some(socket_path.clone()), None, "info".to_string()).unwrap();

        assert_eq!(config.socket_path.to_string_lossy(), socket_path);
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_server_config_with_default_path() {
        let config = ServerConfig::new(None, None, "debug".to_string());

        // Should not fail (will use fallback if needed)
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.log_level, "debug");

        // Path should exist and be a directory
        assert!(
            config.socket_path.exists() || config.socket_path == std::path::PathBuf::from("/tmp")
        );
    }

    #[test]
    fn test_resolve_socket_path_with_provided() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir
            .path()
            .join("test_socket_dir")
            .to_string_lossy()
            .to_string();

        // Ensure it doesn't exist before we call the function
        assert!(!std::path::Path::new(&socket_path).exists());

        let resolved = ServerConfig::resolve_socket_path(Some(socket_path.clone())).unwrap();
        assert_eq!(resolved.to_string_lossy(), socket_path);

        // Verify that the directory was created
        assert!(resolved.is_dir());
    }
}
