//! Fixed direct login and explicit selection routing. Custom choices never authorize acquisition.
use crate::{
    Result,
    game::{LocalGame, LoginProfile},
};

// Match the simulator's portable_bot fields in src/client/network.rs::login_packet.
const PROFILE: LoginProfile = LoginProfile {
    client_version: 291,
    page_echo: [
        54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106, 236, 8, 208, 136, 213, 140,
        111,
    ],
};

pub enum Connection<T> {
    Direct(String),
    Official(T),
}
/// Custom selections never invoke the official acquisition callback.
pub fn prepare_selection<T>(
    selection: &slither_client::server_picker::ServerPreference,
    acquire: impl FnOnce(Option<u16>) -> Result<T>,
) -> Result<Connection<T>> {
    use slither_client::server_picker::ServerPreference;
    match selection {
        ServerPreference::Custom(address) => {
            crate::official::explicit_endpoint(address).map(Connection::Direct)
        }
        choice => acquire(choice.official_id()).map(Connection::Official),
    }
}

/// Interactive and headless desktop endpoints share this fixed profile; library Play stays zero.
pub fn play(game: &mut LocalGame, endpoint: &str, nickname: &str, skin: u8) -> Result<u16> {
    game.play_with_profile(endpoint, nickname, skin, &PROFILE)?;
    Ok(PROFILE.client_version)
}
/// Native interactive custom preferences use the same fixed profile and exact trailing bytes.
pub fn play_appearance(
    game: &mut LocalGame,
    endpoint: &str,
    nickname: &str,
    appearance: &slither_client::skin_studio::AppearanceSelection,
) -> Result<u16> {
    game.play_with_appearance(endpoint, nickname, appearance, &PROFILE)?;
    Ok(PROFILE.client_version)
}
