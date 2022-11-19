use crate::spec::{CodeModel, LinkerFlavor, LldFlavor, PanicStrategy, RelocModel};
use crate::spec::{Target, TargetOptions};

pub fn target() -> Target {
    Target {
        data_layout: "e-m:e-p:64:64-i64:64-i128:128-n64-S128".into(),
        llvm_target: "riscv64".into(),
        pointer_width: 64,
        arch: "riscv64".into(),

        options: TargetOptions {
            linker_flavor: LinkerFlavor::Lld(LldFlavor::Ld),
            linker: Some("rust-lld".into()),
            llvm_abiname: "lp64d".into(),
            cpu: "generic-rv64".into(),
            max_atomic_width: Some(64),
            features: "+m,+a,+f,+d,+c".into(),
            panic_strategy: PanicStrategy::Abort,
            relocation_model: RelocModel::Static,
            code_model: Some(CodeModel::Medium),
            emit_debug_gdb_scripts: false,
            eh_frame_header: false,
            ..Default::default()
        },
    }
}
