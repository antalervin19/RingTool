use super::menu::{split_image_audio, AUDIO_FORMATS, IMAGE_FORMATS};
use super::settings;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const MODEL_FORMATS: [&str; 7] = ["glb", "gltf", "fbx", "obj", "stl", "dae", "ply"];

const BLENDER_SCRIPT: &str = r#"
import bpy, sys

args = sys.argv[sys.argv.index("--") + 1:]
src, dst = args
src_ext = src.rsplit(".", 1)[-1].lower()
dst_ext = dst.rsplit(".", 1)[-1].lower()

bpy.ops.wm.read_factory_settings(use_empty=True)

importers = {
    "glb": lambda: bpy.ops.import_scene.gltf(filepath=src),
    "gltf": lambda: bpy.ops.import_scene.gltf(filepath=src),
    "fbx": lambda: bpy.ops.import_scene.fbx(filepath=src),
    "obj": lambda: bpy.ops.wm.obj_import(filepath=src),
    "stl": lambda: bpy.ops.wm.stl_import(filepath=src),
    "dae": lambda: bpy.ops.wm.collada_import(filepath=src),
    "ply": lambda: bpy.ops.wm.ply_import(filepath=src),
}
exporters = {
    "glb": lambda: bpy.ops.export_scene.gltf(filepath=dst, export_format="GLB"),
    "gltf": lambda: bpy.ops.export_scene.gltf(filepath=dst, export_format="GLTF_SEPARATE"),
    "fbx": lambda: bpy.ops.export_scene.fbx(filepath=dst),
    "obj": lambda: bpy.ops.wm.obj_export(filepath=dst),
    "stl": lambda: bpy.ops.wm.stl_export(filepath=dst),
    "dae": lambda: bpy.ops.wm.collada_export(filepath=dst),
    "ply": lambda: bpy.ops.wm.ply_export(filepath=dst),
}

importers[src_ext]()
exporters[dst_ext]()
"#;

fn ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn unique_output(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{stem}.{extension}"));
    let mut n = 1;

    while candidate.exists() {
        candidate = dir.join(format!("{stem}_{n}.{extension}"));
        n += 1;
    }

    candidate
}

fn run(mut command: Command) -> Result<(), String> {
    let output = command
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to start {:?}: {e}", command.get_program()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(8).collect();
        Err(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
    }
}

pub fn convert(files: &[PathBuf], option_id: &str, output_format: &str) -> Result<PathBuf, String> {
    if option_id.starts_with("slideshow_") {
        let (image, audio) =
            split_image_audio(files).ok_or("expected exactly one image and one audio file")?;
        return image_audio_to_video(image, audio, output_format);
    }

    let [input] = files else {
        return Err("expected a single file".into());
    };

    let input_ext = ext(input);
    let dir = input.parent().unwrap_or(Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let output = unique_output(dir, stem, output_format);

    if MODEL_FORMATS.contains(&input_ext.as_str()) {
        convert_model(input, &output)?;
    } else if IMAGE_FORMATS.contains(&input_ext.as_str())
        || AUDIO_FORMATS.contains(&input_ext.as_str())
    {
        let tools = settings::get();
        let mut cmd = Command::new(&tools.ffmpeg);
        cmd.args(["-hide_banner", "-n", "-i"]).arg(input);
        if output_format == "gif" || output_format == "ico" || output_format == "tiff" {
            cmd.args(["-frames:v", "1"]);
        }
        if output_format == "ico" {
            cmd.args([
                "-vf".to_string(),
                format!(
                    "scale=w={0}:h={0}:force_original_aspect_ratio=decrease",
                    tools.ico_size
                ),
            ]);
        }
        apply_image_quality(&mut cmd, output_format, &tools);
        apply_audio_quality(&mut cmd, output_format, &tools);
        run_ffmpeg_output(cmd, &output)?;
    } else {
        let tools = settings::get();
        let mut cmd = Command::new(&tools.ffmpeg);
        cmd.args(["-hide_banner", "-n", "-i"]).arg(input);

        if option_id.starts_with("images_") {
            cmd.args(["-frames:v", "1"]);
            apply_image_quality(&mut cmd, output_format, &tools);
        } else if option_id.starts_with("audio_") {
            cmd.arg("-vn");
            apply_audio_quality(&mut cmd, output_format, &tools);
        } else {
            apply_video_quality(&mut cmd, output_format, &tools);
            apply_audio_quality(&mut cmd, output_format, &tools);
        }
        run_ffmpeg_output(cmd, &output)?;
    }

    Ok(output)
}

fn apply_audio_quality(cmd: &mut Command, format: &str, tools: &settings::Settings) {
    if matches!(format, "mp3" | "aac" | "ogg" | "opus" | "m4a") {
        cmd.args(["-b:a".to_string(), format!("{}k", tools.audio_kbps)]);
    }
}

fn apply_image_quality(cmd: &mut Command, format: &str, tools: &settings::Settings) {
    match format {
        "jpg" => {
            let q = 2 + (100 - tools.image_quality.min(100)) * 29 / 100;
            cmd.args(["-q:v".to_string(), q.to_string()]);
        }
        "webp" => {
            cmd.args(["-quality".to_string(), tools.image_quality.to_string()]);
        }
        _ => {}
    }
}

fn apply_video_quality(cmd: &mut Command, format: &str, tools: &settings::Settings) {
    let crf = tools.video_crf.to_string();

    match format {
        "mp4" | "mkv" | "mov" => {
            cmd.args(["-c:v", "libx264", "-crf", &crf]);
        }
        "webm" => {
            cmd.args(["-c:v", "libvpx-vp9", "-crf", &crf, "-b:v", "0"]);
        }
        _ => {}
    }
}

pub fn download_url(
    url: &str,
    option_id: &str,
    format: &str,
    target_dir: Option<PathBuf>,
) -> Result<PathBuf, String> {
    let dir = target_dir.unwrap_or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(|p| PathBuf::from(p).join("Downloads"))
            .unwrap_or_else(std::env::temp_dir)
    });

    let tools = settings::get();
    let mut cmd = Command::new(&tools.ytdlp);
    cmd.args(["--no-playlist", "--windows-filenames", "--no-simulate", "--ffmpeg-location"])
        .arg(&tools.ffmpeg)
        .args(["--print", "after_move:filepath", "-P"])
        .arg(&dir)
        .args(["-o", "%(title).150B.%(ext)s"]);

    if option_id.starts_with("url_audio_") {
        cmd.args(["-x", "--audio-format", format]);
        if matches!(format, "mp3" | "m4a" | "opus") {
            cmd.arg("--audio-quality").arg(format!("{}K", tools.audio_kbps));
        }
    } else {
        cmd.args(["-f", "bv*+ba/b", "--merge-output-format", format]);
    }
    cmd.arg(url);

    let output = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to start yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("yt-dlp failed");
        return Err(last.to_string());
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| PathBuf::from(l.trim()))
        .ok_or_else(|| "yt-dlp produced no output file".to_string())
}

fn run_ffmpeg_output(mut cmd: Command, output: &Path) -> Result<(), String> {
    cmd.arg(output);
    run(cmd)
}

fn image_audio_to_video(image: &Path, audio: &Path, format: &str) -> Result<PathBuf, String> {
    let dir = audio.parent().unwrap_or(Path::new("."));
    let stem = audio
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let output = unique_output(dir, stem, format);

    let tools = settings::get();
    let mut cmd = Command::new(&tools.ffmpeg);
    cmd.args(["-hide_banner", "-n", "-loop", "1", "-framerate", "2", "-i"])
        .arg(image)
        .arg("-i")
        .arg(audio)
        .args([
            "-vf",
            "scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p",
            "-c:v",
            "libx264",
            "-crf",
        ])
        .arg(tools.video_crf.to_string())
        .args(["-tune", "stillimage", "-c:a", "aac", "-b:a"])
        .arg(format!("{}k", tools.audio_kbps))
        .arg("-shortest");

    if format == "mp4" || format == "mov" {
        cmd.args(["-movflags", "+faststart"]);
    }

    run_ffmpeg_output(cmd, &output)?;
    Ok(output)
}

fn convert_model(input: &Path, output: &Path) -> Result<(), String> {
    let script = std::env::temp_dir().join("ringtool_blender_convert.py");
    std::fs::write(&script, BLENDER_SCRIPT).map_err(|e| e.to_string())?;

    let mut cmd = Command::new(settings::get().blender);
    cmd.args(["-b", "--factory-startup", "--python"])
        .arg(&script)
        .arg("--")
        .arg(input)
        .arg(output);

    let result = run(cmd);
    let _ = std::fs::remove_file(&script);
    result?;

    if output.exists() {
        Ok(())
    } else {
        Err("Blender finished without producing an output file".into())
    }
}
