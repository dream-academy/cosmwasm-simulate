use cc::Build;
use walkdir::WalkDir;

fn main() {
    let mut cfg = Build::new();
    cfg.compiler("clang");
    cfg.flag("-nostdlibinc");
    cfg.flag("-fno-stack-protector");
    cfg.flag("-fno-profile-instr-generate");
    cfg.flag("-fno-coverage-mapping");
    cfg.define("COMPILER_RT_HAS_ATOMICS", "1");

    let sources = [
        "c/InstrProfiling.c",
        "c/InstrProfilingBuffer.c",
        "c/InstrProfilingInternal.c",
        "c/InstrProfilingMerge.c",
        "c/InstrProfilingPlatformLinux.c",
        "c/InstrProfilingWriter.c",
        "c/InstrProfilingVersionVar.c",
    ];

    for source in &sources {
        cfg.file(source);
    }

    cfg.compile("llvm_profiler_runtime");

    for entry in WalkDir::new("c") {
        println!(
            "cargo:rerun-if-changed={}",
            entry.unwrap().path().to_str().unwrap()
        );
    }
}
