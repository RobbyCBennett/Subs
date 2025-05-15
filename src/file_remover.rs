use std::path::Path;


#[derive(Debug)]
pub struct FileRemover<'a>
{
	path: &'a Path,
}


impl<'a> FileRemover<'a>
{
	pub fn new(path: &'a Path) -> FileRemover<'a>
	{
		return FileRemover { path };
	}
}


impl<'a> Drop for FileRemover<'a>
{
	fn drop(&mut self)
	{
		let _ = std::fs::remove_file(self.path);
	}
}
