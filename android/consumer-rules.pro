# Rust reaches these classes by name through JNI, so R8 can't see that they are used.
-keep class app.kabar.** { *; }
