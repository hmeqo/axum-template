//! 生成 ConnectRPC 的消息类型与服务代码，并导出描述符集（`/api-docs/rpc` 的 gRPC 反射用）。

fn main() {
    connectrpc_build::Config::new()
        .files(&["proto/template/v1/template.proto"])
        .includes(&["proto/"])
        .include_file("_connectrpc.rs")
        .emit_descriptor_set("template.fds.bin")
        .compile()
        .expect("failed to compile the proto contract");
}
