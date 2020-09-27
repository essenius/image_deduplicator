use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::fs::{self,File, OpenOptions};
use sha2::{Sha256, Digest};
use std::error::Error;
use filetime::FileTime;
use walkdir::WalkDir;

static DUPLICATE_EXTENSION: &str = "duplicate";

#[derive(Debug, Clone)]
struct ImageData {
    path: String,
    create_time: FileTime,
    size: u64,
    is_duplicate: bool,
}

fn hash(file: &String) -> Result<String, io::Error> {
    let path = Path::new(file);

    let mut file = File::open(&path)?;
    let mut sha256 = Sha256::new();
    io::copy(&mut file, &mut sha256).expect("copy failed");
    Ok(format!("{:x}", sha256.finalize()))
}

fn add_to_logfile(original: &String, duplicate: &String) {
    let dup_file = Path::new(duplicate);
    let logfile_path = dup_file.parent().unwrap().join("duplicates.log");
    println!("{}",logfile_path.display());
    let logfile = OpenOptions::new()
            .append(true)
            .create(true)
            .open(logfile_path)
            .unwrap();
    let log_line = format!("{} is duplicate of {}", &duplicate, &original);
    writeln!(&logfile, "{}", &log_line).unwrap();    
    println!("{}", &log_line);    
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    if !&path.exists() {
        println!("Path '{}' does not exist", path.clone().into_os_string().into_string().unwrap());
    }

    let mut images : Vec<ImageData> = Vec::new();
    let mut duplicate_count = 0;
    for entry in WalkDir::new(path) { //entry in fs::read_dir(path)?  {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            let create_time = FileTime::from_creation_time(&metadata).unwrap();
            if  FileTime::from_last_modification_time(&metadata)== FileTime::zero() {
                println!("Setting modified time to {}", &create_time);
                filetime::set_file_mtime(&path, create_time).unwrap();
            }
            let name = format!("{}",path.display());
            let is_duplicate = path.extension().unwrap() == DUPLICATE_EXTENSION;
            if is_duplicate {
                duplicate_count +=1;
            }
            images.push(ImageData{path: name.clone(), size: metadata.len(), create_time: create_time, is_duplicate: is_duplicate});
            print!("{}", if is_duplicate {"#"} else {"."});
        }
        io::stdout().flush().unwrap();
    }
    println!("Found {} files, {} existing duplicates.", &images.len(), duplicate_count);

    images.sort_by(|a, b| a.size.cmp(&b.size).then(a.create_time.cmp(&b.create_time)));
    duplicate_count = 0;
    let mut duplicate_size = 0;
    for i in 0..images.len() {
        let base_entry = images[i].clone();
        if base_entry.is_duplicate {
            continue;
        }
        let mut j = i+1; 
        let mut base_hash_calculated = false;
        let mut base_hash = String::from("");
        while j < images.len() && images[j].size == base_entry.size {
            if !images[j].is_duplicate { 
                if !base_hash_calculated {
                    base_hash = hash(&base_entry.path).unwrap();
                    base_hash_calculated = true;
                }
                let potential_dup_hash = hash(&images[j].path).unwrap();
                if potential_dup_hash.eq(&base_hash) {
                    images[j].is_duplicate = true;
                    duplicate_count += 1;
                    duplicate_size += images[j].size;                
                    let new_duplicate_name = format!("{}.{}", &images[j].path, DUPLICATE_EXTENSION);
                    fs::rename(&images[j].path, &new_duplicate_name).unwrap(); 
                    add_to_logfile(&base_entry.path, &new_duplicate_name);
                }
            } else {
                println!("Skipping duplicate {}", images[j].path);
            }
            j+=1;
        }
    }
    println!("New duplicates found: {}, total size: {}", duplicate_count, duplicate_size);
    Ok(())
}
