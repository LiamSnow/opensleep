use super::*;

// TODO more testing (esp for MQTT)

#[tokio::test]
async fn test_load_solo_config() {
    let config = Config::load("example_solo.ron").await.unwrap();
    assert_eq!(config.timezone.iana_name().unwrap(), "America/New_York");
    assert!(!config.away_mode);
    match &config.profile {
        SidesConfig::Solo(profile) => {
            assert_eq!(profile.temperatures, vec![27., 29., 31.]);
        }
        _ => panic!("Expected solo profile"),
    }
}

#[tokio::test]
async fn test_load_couples_config() {
    let config = Config::load("example_couples.ron").await.unwrap();
    assert_eq!(config.timezone.iana_name().unwrap(), "America/New_York");
    assert!(!config.away_mode);
    match &config.profile {
        SidesConfig::Couples { left, right } => {
            assert_eq!(left.temperatures, vec![27., 29., 31.]);
            assert_eq!(right.temperatures, vec![27., 29., 31.]);
        }
        _ => panic!("Expected couples profile"),
    }
}
