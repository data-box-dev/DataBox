use tauri::{LogicalPosition, TitleBarStyle, WebviewUrl, WebviewWindowBuilder};
mod state;
mod connection_registry;
mod commands;
use state::AppState;
use connection_registry::ConnectionRegistry;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState::new())
        .manage(ConnectionRegistry::new())
        .setup(|app| {
            let win_builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .decorations(true)
                    .hidden_title(true)
                    .inner_size(1300.0, 900.0)
                    .min_inner_size(300.0, 200.0);

            // 仅在 macOS 时设置透明标题栏
            #[cfg(target_os = "macos")]
            win_builder
                .title_bar_style(TitleBarStyle::Overlay)
                .traffic_light_position(LogicalPosition::new(15, 22))
                .build()
                .unwrap();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler\![
            // connection
            connection::list_connections,
            connection::register_connection,
            connection::get_connection,
            connection::unregister_connection,
            connection::test_connection,
            connection::save_connection,
            connection::load_password,
            // database
            database::connect,
            database::ping,
            database::disconnect,
            database::switch_database,
            // schema
            schema::list_databases,
            schema::list_schemas,
            schema::list_tables,
            schema::list_columns,
            schema::describe_table,
            // query
            query::execute_sql,
            query::execute_batch,
            query::execute_multi,
            // config
            config::save_connections,
            config::load_connections,
        ])
        .run(tauri::generate_context\!())
        .expect("error while running tauri application");
}
