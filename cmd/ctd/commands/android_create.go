package commands

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// CreateAndroid implements the 'ctd create-android' command
func CreateAndroid(args []string) error {
	fs := flag.NewFlagSet("create-android", flag.ExitOnError)
	name := fs.String("name", "", "App name (defaults to project name from centered.toml)")
	outputDir := fs.String("output", "android", "Output directory for Android project")
	force := fs.Bool("force", false, "Overwrite existing files")
	fs.Parse(args)

	// Load project config
	config, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	// Use provided name or fall back to config
	appName := *name
	if appName == "" {
		appName = config.App.Name
	}

	// Sanitize app name
	safeName := sanitizeName(appName)

	// Check if output directory exists
	if _, err := os.Stat(*outputDir); err == nil && !*force {
		return fmt.Errorf("directory %s already exists (use --force to overwrite)", *outputDir)
	}

	fmt.Printf("Creating Android project for %s...\n", appName)

	// Template data
	data := AndroidTemplateData{
		AppName:     appName,
		SafeName:    safeName,
		PackageName: config.Android.PackageName,
		PackagePath: strings.ReplaceAll(config.Android.PackageName, ".", "/"),
		MinSDK:      config.Android.MinSDK,
		TargetSDK:   config.Android.TargetSDK,
		Version:     config.App.Version,
	}

	// Create directory structure
	dirs := []string{
		*outputDir,
		filepath.Join(*outputDir, "app"),
		filepath.Join(*outputDir, "app", "src", "main"),
		filepath.Join(*outputDir, "app", "src", "main", "java", data.PackagePath),
		filepath.Join(*outputDir, "app", "src", "main", "jniLibs", "arm64-v8a"),
		filepath.Join(*outputDir, "app", "src", "main", "jniLibs", "armeabi-v7a"),
		filepath.Join(*outputDir, "app", "src", "main", "jniLibs", "x86_64"),
		filepath.Join(*outputDir, "app", "src", "main", "res", "values"),
		filepath.Join(*outputDir, "gradle", "wrapper"),
	}
	for _, dir := range dirs {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
	}

	// Generate files
	files := map[string]string{
		filepath.Join(*outputDir, "settings.gradle"):                                         androidSettingsGradleTemplate,
		filepath.Join(*outputDir, "build.gradle"):                                            androidRootBuildGradleTemplate,
		filepath.Join(*outputDir, "gradle.properties"):                                       androidGradlePropertiesTemplate,
		filepath.Join(*outputDir, "app", "build.gradle"):                                     androidAppBuildGradleTemplate,
		filepath.Join(*outputDir, "app", "src", "main", "AndroidManifest.xml"):               androidManifestTemplate,
		filepath.Join(*outputDir, "app", "src", "main", "java", data.PackagePath, "MainActivity.java"): androidMainActivityTemplate,
		filepath.Join(*outputDir, "app", "src", "main", "res", "values", "strings.xml"):      androidStringsTemplate,
		filepath.Join(*outputDir, "README.md"):                                               androidReadmeTemplate,
	}

	for path, tmplStr := range files {
		if err := writeTemplate(path, tmplStr, data); err != nil {
			return fmt.Errorf("failed to write %s: %w", path, err)
		}
		fmt.Printf("  ✓ Created %s\n", path)
	}

	fmt.Println("")
	fmt.Printf("✓ Android project created in %s/\n", *outputDir)
	fmt.Println("")
	fmt.Println("Next steps:")
	fmt.Println("  1. Build the Rust engine for Android:")
	fmt.Println("     ctd build-android")
	fmt.Println("  2. Open in Android Studio:")
	fmt.Printf("     studio %s\n", *outputDir)
	fmt.Println("  3. Or run directly on emulator:")
	fmt.Println("     ctd run-android")

	return nil
}

type AndroidTemplateData struct {
	AppName     string
	SafeName    string
	PackageName string
	PackagePath string
	MinSDK      int
	TargetSDK   int
	Version     string
}

// Android project templates
const androidSettingsGradleTemplate = `rootProject.name = "{{.AppName}}"
include ':app'
`

const androidRootBuildGradleTemplate = `// Top-level build file
buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath 'com.android.tools.build:gradle:8.1.0'
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

task clean(type: Delete) {
    delete rootProject.buildDir
}
`

const androidGradlePropertiesTemplate = `org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
`

const androidAppBuildGradleTemplate = `plugins {
    id 'com.android.application'
}

android {
    namespace '{{.PackageName}}'
    compileSdk {{.TargetSDK}}

    defaultConfig {
        applicationId "{{.PackageName}}"
        minSdk {{.MinSDK}}
        targetSdk {{.TargetSDK}}
        versionCode 1
        versionName "{{.Version}}"

        ndk {
            abiFilters 'arm64-v8a', 'armeabi-v7a', 'x86_64'
        }
    }

    buildTypes {
        release {
            minifyEnabled false
            proguardFiles getDefaultProguardFile('proguard-android-optimize.txt'), 'proguard-rules.pro'
        }
    }

    compileOptions {
        sourceCompatibility JavaVersion.VERSION_1_8
        targetCompatibility JavaVersion.VERSION_1_8
    }

    sourceSets {
        main {
            jniLibs.srcDirs = ['src/main/jniLibs']
        }
    }
}

dependencies {
    implementation 'androidx.appcompat:appcompat:1.6.1'
    implementation 'com.google.android.material:material:1.9.0'
}
`

const androidManifestTemplate = `<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">

    <uses-feature android:glEsVersion="0x00030000" android:required="true" />
    <uses-permission android:name="android.permission.INTERNET" />

    <application
        android:label="{{.AppName}}"
        android:hasCode="true"
        android:theme="@style/Theme.AppCompat.Light.NoActionBar">

        <activity
            android:name=".MainActivity"
            android:label="{{.AppName}}"
            android:configChanges="orientation|keyboardHidden|screenSize|uiMode|colorMode"
            android:exported="true">

            <meta-data android:name="android.app.lib_name" android:value="centered_engine" />

            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
`

const androidMainActivityTemplate = `package {{.PackageName}};

import android.app.NativeActivity;
import android.os.Bundle;

/**
 * Main activity that hosts the Centered app.
 * This activity loads the native Rust library and delegates to it.
 */
public class MainActivity extends NativeActivity {
    static {
        // Load the Rust library
        System.loadLibrary("centered_engine");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
    }
}
`

const androidStringsTemplate = `<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="app_name">{{.AppName}}</string>
</resources>
`

const androidReadmeTemplate = `# {{.AppName}} - Android

This Android project was generated by ` + "`ctd create-android`" + `.

## Building

### Prerequisites

1. Android Studio installed (or Android SDK + NDK)
2. Rust toolchain with Android targets:
   ` + "```bash" + `
   rustup target add aarch64-linux-android    # ARM64 devices
   rustup target add armv7-linux-androideabi  # ARM32 devices
   rustup target add x86_64-linux-android     # x86_64 emulators
   ` + "```" + `
3. Environment variables:
   ` + "```bash" + `
   export ANDROID_HOME=$HOME/Library/Android/sdk
   export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
   ` + "```" + `

### Build for Android

` + "```bash" + `
ctd build-android
` + "```" + `

This will:
1. Build the Rust engine for Android targets
2. Copy the .so files to android/app/src/main/jniLibs/

### Run on Emulator

` + "```bash" + `
ctd run-android
` + "```" + `

### Build APK

` + "```bash" + `
cd android
./gradlew assembleDebug
` + "```" + `

The APK will be at ` + "`app/build/outputs/apk/debug/app-debug.apk`" + `

## Project Structure

` + "```" + `
android/
├── app/
│   ├── src/main/
│   │   ├── java/{{.PackagePath}}/
│   │   │   └── MainActivity.java
│   │   ├── jniLibs/
│   │   │   ├── arm64-v8a/
│   │   │   │   └── libcentered_engine.so
│   │   │   ├── armeabi-v7a/
│   │   │   │   └── libcentered_engine.so
│   │   │   └── x86_64/
│   │   │       └── libcentered_engine.so
│   │   ├── res/
│   │   │   └── values/strings.xml
│   │   └── AndroidManifest.xml
│   └── build.gradle
├── build.gradle
├── settings.gradle
└── README.md
` + "```" + `

## Configuration

Edit ` + "`centered.toml`" + ` in your project root to configure:

` + "```toml" + `
[android]
min_sdk = {{.MinSDK}}
target_sdk = {{.TargetSDK}}
package_name = "{{.PackageName}}"
` + "```" + `
`
