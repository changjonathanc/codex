use super::FinalMessageColor;
use crate::config_toml::ConfigToml;
use pretty_assertions::assert_eq;

#[test]
fn final_message_color_round_trips_and_defaults() {
    let config: ConfigToml = toml::from_str("[tui]\nfinal_message_color = '#89B4fA'\n").unwrap();
    let color = config.tui.unwrap().final_message_color.unwrap();
    assert_eq!(color.rgb(), [137, 180, 250]);
    assert_eq!(serde_json::to_string(&color).unwrap(), "\"#89b4fa\"");
    assert_eq!(
        serde_json::from_str::<FinalMessageColor>("\"#89b4fa\"").unwrap(),
        color
    );
    let config: ConfigToml = toml::from_str("[tui]\n").unwrap();
    let tui = config.tui.unwrap();
    assert_eq!(tui.final_message_color, None);
    assert!(
        serde_json::to_value(tui)
            .unwrap()
            .get("final_message_color")
            .is_none()
    );
}

#[test]
fn final_message_color_rejects_invalid_config_values() {
    for value in [
        "blue",
        "89b4fa",
        "#abc",
        "#89b4faff",
        "#89b4fg",
        "#ééé",
        "#89b4fa ",
    ] {
        let config = format!("[tui]\nfinal_message_color = '{value}'\n");
        let error = toml::from_str::<ConfigToml>(&config)
            .unwrap_err()
            .to_string();
        assert!(error.contains("final_message_color must be a six-digit hex color"));
    }
}
