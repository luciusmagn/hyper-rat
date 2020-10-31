extern crate ramhorns;
extern crate fs_extra;
extern crate regex;
extern crate toml;

use ramhorns::{Template, Ramhorns};
use regex::{Captures, Regex};
use fs_extra::dir;

use std::fs::{read_dir, read_to_string, create_dir, remove_dir_all, write};
use std::io::ErrorKind as IoError;
use std::collections::HashMap;
use std::process::exit;
use std::path::PathBuf;
use std::error::Error;

static TEMPLATE: &str = "{{body}}";

fn main() -> Result<(), Box<dyn Error>> {
	let content_regex = Regex::new(
		r#"\[\[(?P<content>(((\.\.?/)|([.a-zA-Z0-9_/\-\\]))+(\.[a-zA-Z0-9]+)?))(?P<template> +(((\.\.?/)|([.a-zA-Z0-9_/\-\\]))+(\.[a-zA-Z0-9]+)?))?\]\]"#,
	)?;
	let mut template_cache = HashMap::new();
	template_cache.insert("base".to_string(), Template::new(TEMPLATE)?);

	let template_files = read_dir("theme")?
		.into_iter()
		.filter_map(|x| x.ok())
		.map(|x| x.path())
		.filter(|x| x.is_file())
		.collect::<Vec<PathBuf>>();

	let mut templates = Ramhorns::from_folder("theme")?;

	remove_dir_all("build").or_else(|x| {
		if x.kind() == IoError::NotFound {
			Ok(())
		} else {
			Err(x)
		}
	})?;

	create_dir("build")?;
	dir::copy("media", "build/", &dir::CopyOptions::new())?;
	dir::copy("theme/static", "build/", &dir::CopyOptions::new())?;

	template_files.iter().for_each(|path| {
		let tpl = templates
			.from_file(&path.strip_prefix("theme").unwrap().display().to_string())
			.unwrap();

		if let Err(e) = tpl.render_to_file(
			&PathBuf::from("build").join(&path.strip_prefix("theme").unwrap()),
			&(),
		) {
			println!("failed to render to file: {}", e);
		}
	});

	let built = read_dir("build")?
		.filter_map(|x| x.ok().map(|x| x.path()))
		.filter(|x| x.is_file())
		.collect::<Vec<PathBuf>>();

	built
		.iter()
		.map(|x| (x, read_to_string(x)))
		.filter_map(|x| if let (n, Ok(s)) = x { Some((n, s)) } else { None })
		.for_each(|(path, contents)| {
			let processed = content_regex.replace_all(&contents, |caps: &Captures| {
				println!("{}", caps.len());
				let content = match read_to_string(dbg!(caps["content"].to_string())) {
					Ok(s) => s,
					Err(e) => {
						eprintln!("failed to read file {}: {}", path.display(), e);
						exit(1);
					}
				};

				let tpl_name = caps
					.name("template")
					.map(|x| x.as_str().trim())
					.unwrap_or("base")
					.to_string();

				let (head, body);
				let v: Vec<&str> = content.splitn(2, "\n\n").collect();

				match v.len() {
					1 => {
						body = v[0].trim();
						head = "".to_string();
					}
					_ => {
						head = v[0].trim().to_string();
						body = v[1].trim();
					},
				}

				let data = match toml::from_str::<HashMap<String, String>>(&head) {
					Ok(mut s) => {
						s.insert("body".into(), body.into());
						s
					},
					Err(_) => {
						let mut h = HashMap::new();
						h.insert("body".into(), body.into());
						h
					},
				};

				let tpl = template_cache.entry(tpl_name.clone()).or_insert_with(|| {
					match read_to_string(&tpl_name) {
						Ok(s) => Template::new(s).unwrap_or_else(|_| {
							eprintln!("template suck");
							exit(1);
						}),
						Err(e) => {
							eprintln!(
								"failed to make template from file {}: {}",
								tpl_name, e
							);
							exit(1);
						},
					}
				});

				tpl.render(&data)
			});

			if let Err(e) = write(path, processed.to_string()) {
				eprintln!("failed to write to file {}: {}", path.display(), e);
			}
		});

	Ok(())
}
