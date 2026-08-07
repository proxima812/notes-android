plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.local.organizer.reminders"
    compileSdk = 36

    defaultConfig {
        // Matches the app. Notification channels and `setAndAllowWhileIdle`
        // both exist from 26, so there is no legacy path to carry.
        minSdk = 26
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.17.0")
    // `ActivityResult` comes from here: the sound picker is an activity
    // result, and Tauri exposes the launcher without re-exporting the type.
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation(project(":tauri-android"))

    testImplementation("junit:junit:4.13.2")
    // The `org.json` bundled with the Android SDK is a stub that throws in unit
    // tests. The real implementation on the test classpath is what lets the
    // alarm journal be tested without an emulator.
    testImplementation("org.json:json:20240303")
}
