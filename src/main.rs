// Copyright 2020 Rik Essenius
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. 
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, 
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions
// and limitations under the License.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::fs::{self,File, OpenOptions};
use sha2::{Sha256, Digest};
use std::error::Error;
use filetime::FileTime;
use walkdir::{DirEntry, WalkDir};
use std::io::ErrorKind;

static DUPLICATE_EXTENSION: &str = "duplicate";

#[derive(Clone)]
struct ImageData {
    path: String,
    create_time: FileTime,
    size: u64,
    hash: Option<String>,
}

impl ImageData {
    fn new(path: &Path) -> ImageData {
        let metadata = fs::metadata(&path).unwrap();
        let create_time = get_create_time(&metadata);
        correct_zero_modification_date(path, &metadata, &create_time);
        let name = format!("{}", path.display());
        ImageData { path: name, size: metadata.len(), create_time, hash: None}
    }

    fn is_duplicate(&self) -> bool {
        let mut is_duplicate = false;
        if let Some(extension) = Path::new(&self.path).extension() {
            is_duplicate = extension == DUPLICATE_EXTENSION;
        }
        is_duplicate
    }

    fn mark_duplicate(&mut self) {
        let new_duplicate_name = format!("{}.{}", &self.path, DUPLICATE_EXTENSION);
        println!("Renaming {} to {}", &self.path, &new_duplicate_name);
        fs::rename(&self.path, &new_duplicate_name).unwrap();
        self.path = new_duplicate_name;
    }

    fn hash(&mut self) -> Result<String, io::Error> {
        match &self.hash {
            None => {
                println!("Calculating hash for {}", &self.path);
                let path = Path::new(&self.path);    
                let mut file = File::open(&path)?;
                let mut sha256 = Sha256::new();
                io::copy(&mut file, &mut sha256).expect("copy failed");
                self.hash = Some(format!("{:x}",sha256.finalize()));
                Ok(self.hash.clone().unwrap())
            },
            Some(hash) => Ok(hash.clone()),
        }
    }
}

fn get_create_time(metadata: &fs::Metadata) -> FileTime {
    let create_time : FileTime ;
    if let Some(time) = FileTime::from_creation_time(metadata) {
        create_time = time;
    } else {
        create_time = FileTime::from_last_modification_time(metadata);
    }
    create_time
}

fn correct_zero_modification_date(path: &Path, metadata: &fs::Metadata, create_time: &filetime::FileTime) {
    if  FileTime::from_last_modification_time(metadata)== FileTime::zero() {
        println!("Setting modified time to {}", create_time);
        filetime::set_file_mtime(path, *create_time).unwrap();
    }
}

struct ImageSet {
    images: Vec<ImageData>,
}

impl ImageSet {
    fn new(folder: &Path) -> ImageSet {
        let mut images : Vec<ImageData> = Vec::new();
        let mut duplicate_count = 0;
        
        let mut walker = WalkDir::new(folder).into_iter();
        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(err)) => { 
                    let path = err.path().unwrap_or(Path::new("")).display();
                    if let Some(inner) = err.io_error() {
                        if inner.kind() == ErrorKind::PermissionDenied {
                            println!("Skipping {}: permission denied.", path);
                            continue;
                        }
                    }
                    panic!("ERROR: {}", err);
                }
                Some(Ok(entry)) => entry,
            };
            if entry.file_type().is_dir()  {
                if is_hidden(&entry) && entry.depth() > 0 {
                    println!("Skipping hidden folder: {}",  entry.path().display());
                    walker.skip_current_dir();
                } 
                continue;
            }
            let image = ImageData::new(entry.path());
            if image.is_duplicate() {
                duplicate_count += 1;
                print!("#");
            } else {
                print!(".");
                images.push(image);
            }
            io::stdout().flush().unwrap();
        }
        println!(" Found {} files, excluding {} existing duplicates.", &images.len(), duplicate_count);
        ImageSet { images }
    }
    
    fn sort(&mut self) {
        self.images.sort_by(|a, b| a.size.cmp(&b.size).then(a.create_time.cmp(&b.create_time)));
    }    

    fn mark_duplicates(&mut self) {
        let mut duplicate_count = 0;
        let mut duplicate_size = 0;
        let mut previous_percentage = 101; // positive number that can't occur
        for base_entry in 0..self.images.len() {
            if self.images[base_entry].is_duplicate() {
                continue;
            }
            // show status per 5 percent (* 20 = * 100 /5)
            let percentage = (base_entry * 20 / &self.images.len()) * 5;
            if previous_percentage != percentage {
                println!("{}", format!("{}%", percentage));
                previous_percentage = percentage;
            }
            let mut candidate_dup = base_entry + 1; 
            while candidate_dup < self.images.len() && &self.images[candidate_dup].size == &self.images[base_entry].size {
                if !&self.images[candidate_dup].is_duplicate() && self.images[candidate_dup].hash().unwrap().eq(&self.images[base_entry].hash().unwrap()) {
                    let _ = self.images[candidate_dup].mark_duplicate();
                    duplicate_count += 1;
                    duplicate_size += &self.images[candidate_dup].size;                
                    add_to_logfile(&self.images[base_entry].path, &self.images[candidate_dup].path);
                }
                candidate_dup += 1;
            }
        }
        println!("New duplicates found: {}, total size: {}", duplicate_count, duplicate_size);
    }
}

fn add_to_logfile(original: &String, duplicate: &String) {
    let dup_file = Path::new(duplicate);
    let logfile_path = dup_file.parent().unwrap().join("duplicates.log");
    let logfile = OpenOptions::new()
            .append(true)
            .create(true)
            .open(logfile_path)
            .unwrap();
    let log_line = format!("{} is duplicate of {}", &duplicate, &original);
    writeln!(&logfile, "{}", &log_line).unwrap();    
    println!("{}", &log_line);    
}

fn is_hidden(entry: &DirEntry) -> bool {    
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with('.'))
         .unwrap_or(false)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    if !&path.exists() {
        println!("Path '{}' does not exist", path.clone().into_os_string().into_string().unwrap());
    }

    let mut images = ImageSet::new(&path);
    images.sort();
    images.mark_duplicates();
    Ok(())
}
