use std::fs;
use std::path::PathBuf;

pub fn find_deploy_files<F>(start_folder: &str, action: &mut F) -> std::io::Result<()>
where
    F: FnMut(PathBuf),
{
    for entry in fs::read_dir(start_folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            find_deploy_files(path.to_str().unwrap(), action)?;
        } else if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
            if file_name.ends_with(".deploy.toml") {
                action(path);
            }
        }
    }
    Ok(())
}
