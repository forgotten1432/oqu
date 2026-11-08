# JNA + UniFFI generated bindings are reached reflectively
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class uniffi.** { *; }
-dontwarn java.awt.**
-keepclassmembers class * { @android.webkit.JavascriptInterface <methods>; }
