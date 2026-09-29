use jni::objects::{JClass, JValue};
use jni::{jni_sig, jni_str, EnvUnowned, JavaVM};
use once_cell::sync::OnceCell;

static GLOBAL_JVM: OnceCell<JavaVM> = OnceCell::new();

pub fn set_java_vm(vm: JavaVM) {
    if GLOBAL_JVM.get().is_some() {
        // 如果已经初始化，直接返回
        println!("JavaVM is already set, skipping initialization");
        return;
    }
    GLOBAL_JVM.set(vm).expect("JavaVM can only be set once");
}

pub fn get_java_vm() -> &'static JavaVM {
    GLOBAL_JVM.get().expect("JavaVM is not initialized")
}

#[no_mangle]
pub extern "system" fn Java_com_meow_app_MainActivity_setRustJNIEnv<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    env.with_env(|env| {
        let java_vm = env.get_java_vm()?;
        set_java_vm(java_vm);
        Ok::<(), jni::errors::Error>(())
    })
    .resolve::<jni::errors::LogErrorAndDefault>();
}

// 从全局环境中调用 Java 方法
pub fn toggle_status_bar_theme(is_dark_mode: bool) -> Result<bool, String> {
    println!("Current Thread ID (set): {:?}", std::thread::current().id());

    get_java_vm()
        .attach_current_thread(|env| -> jni::errors::Result<bool> {
            let main_activity_class = env.find_class(jni_str!("com/meow/app/MainActivity"))?;
            let activity_instance = env
                .call_static_method(
                    main_activity_class,
                    jni_str!("getInstance"),
                    jni_sig!("()Lcom/meow/app/MainActivity;"),
                    &[],
                )?
                .l()?;

            env.call_method(
                activity_instance,
                jni_str!("toggleStatusBarTextColor"),
                jni_sig!("(Z)Z"),
                &[JValue::from(is_dark_mode)],
            )?
            .z()
        })
        .map_err(|error| format!("Failed to call Java method: {error:?}"))
}
