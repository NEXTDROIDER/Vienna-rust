use vienna_vma::{export_mcemod, HostApi, ModResult};

fn on_load(api: &HostApi) -> ModResult {
    api.log_info("hello-mcemod loaded through VMA");
    api.log_debug(&format!("host ABI version: {}", api.abi_version()));
    Ok(())
}

fn on_unload(api: &HostApi) {
    api.log_info("hello-mcemod unloading");
}

fn on_server_start(api: &HostApi) {
    api.log_info("hello-mcemod received on_server_start");
}

fn on_server_stop(api: &HostApi) {
    api.log_info("hello-mcemod received on_server_stop");
}

fn on_player_join(api: &HostApi, player_name: &str) {
    api.log_info(&format!("player joined: {player_name}"));
}

fn on_player_leave(api: &HostApi, player_name: &str) {
    api.log_info(&format!("player left: {player_name}"));
}

fn on_command(api: &HostApi, player_name: &str, command: &str) {
    api.log_info(&format!("command from {player_name}: {command}"));
}

export_mcemod!(
    name: "hello-mcemod",
    version: "0.1.0",
    on_load: on_load,
    on_unload: on_unload,
    on_server_start: on_server_start,
    on_server_stop: on_server_stop,
    on_player_join: on_player_join,
    on_player_leave: on_player_leave,
    on_command: on_command,
);
