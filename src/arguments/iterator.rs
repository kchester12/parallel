use super::disk_buffer::*;
use super::errors::{FileErr, InputIteratorErr};
use std::path::{Path, PathBuf};

/// The `InputIterator` tracks the total number of arguments, the current argument counter, and
/// takes ownership of an `InputBuffer` which buffers input arguments from the disk when arguments
/// stored in memory are depleted.
pub struct InputIterator {
    pub total_arguments: usize,
    pub curr_argument:   usize,
    input_buffer:        InputBuffer,
}

impl InputIterator {
    pub fn new(path: &Path, args: usize) -> Result<InputIterator, FileErr> {
        // Create an `InputBuffer` from the unprocessed file.
        let disk_buffer = try!(DiskBuffer::new(path).read()
            .map_err(|why| FileErr::Open(PathBuf::from(path), why)));
        let input_buffer = try!(InputBuffer::new(disk_buffer));

        Ok(InputIterator {
            total_arguments:   args,
            curr_argument:     0,
            input_buffer:      input_buffer,
        })
    }

    fn buffer(&mut self) -> Result<(), InputIteratorErr> {
        // Read the next set of arguments from the unprocessed file, but only read as many bytes
        // as the buffer can hold without overwriting the unused bytes that was shifted to the left.
        try!(self.input_buffer.disk_buffer.buffer(self.input_buffer.capacity).map_err(|why| {
            InputIteratorErr::FileRead(PathBuf::from(self.input_buffer.disk_buffer.path.clone()), why)
        }));
        let bytes_read = self.input_buffer.disk_buffer.capacity;

        // Update the recorded number of arguments and indices.
        self.input_buffer.start = self.input_buffer.end + 1;
        self.input_buffer.count_arguments(bytes_read);
        self.input_buffer.index = 0;
        Ok(())
    }
}

// Implement the `Iterator` trait for `InputIterator` to gain access to all the `Iterator` methods for free.
impl Iterator for InputIterator {
    type Item = Result<String, InputIteratorErr>;

    fn next(&mut self) -> Option<Result<String, InputIteratorErr>> {
        if self.curr_argument == self.total_arguments {
            // If all arguments have been depleted, return `None`.
            return None
        } else if self.curr_argument == self.input_buffer.end {
            // If the next argument is not stored in the internal buffer, update the buffer.
            if let Err(err) = self.buffer() { return Some(Err(err)); }
        }

        // Obtain the start and end indices to know where to find the input in the array.
        let end   = self.input_buffer.indices[self.input_buffer.index + 1];
        let start = if self.input_buffer.index == 0 {
            self.input_buffer.indices[self.input_buffer.index]
        } else {
            self.input_buffer.indices[self.input_buffer.index] + 1
        };

        // Increment the iterator's state.
        self.curr_argument += 1;
        self.input_buffer.index  += 1;

        // Copy the input from the buffer into a `String` and return it
        Some(Ok(String::from_utf8_lossy(&self.input_buffer.disk_buffer.data[start..end]).into_owned()))
    }
}

/// Higher level buffer implementation which keeps track of how many inputs are currently
/// stored in the buffer, where all of the indices of the input delimiters are, and which
/// segment of the complete set of arguments are currently buffered.
struct InputBuffer {
    index:       usize,
    start:       usize,
    end:         usize,
    capacity:    usize,
    disk_buffer: DiskBufferReader,
    indices:     [usize; BUFFER_SIZE / 2],
}

impl InputBuffer {
    /// Takes ownership of a `DiskBufferReader` and transforms it into a higher level
    /// `InputBuffer` which will track additional information about the disk buffer.
    fn new(mut unprocessed: DiskBufferReader) -> Result<InputBuffer, FileErr> {
        try!(unprocessed.buffer(0).map_err(|why| FileErr::Read(unprocessed.path.clone(), why)));
        let bytes_read = unprocessed.capacity;

        let mut temp = InputBuffer {
            index:       0,
            start:       0,
            end:         0,
            capacity:    0,
            disk_buffer: unprocessed,
            indices:     [0usize; BUFFER_SIZE / 2]
        };

        temp.count_arguments(bytes_read);
        Ok(temp)
    }

    /// Counts the number of arguments that are stored in the buffer, marking the location of
    /// the indices and the actual capacity of the buffer's useful information.
    fn count_arguments(&mut self, bytes_read: usize) {
        self.capacity = 0;
        for (id, byte) in self.disk_buffer.data.iter().take(bytes_read).enumerate() {
            if *byte == b'\n' {
                self.indices[self.capacity + 1] = id;
                self.capacity += 1;
                self.end += 1;
            }
        }
        self.capacity = self.indices[self.capacity];
    }
}

#[test]
fn test_input_iterator() {
    let iterator = InputIterator::new(Path::new("tests/buffer.dat"), 4096).unwrap();
    assert_eq!(0,  iterator.input_buffer.start);
    assert_eq!(1859, iterator.input_buffer.end);
    for (actual, expected) in iterator.zip((1..4096)) {
        assert_eq!(actual.unwrap(), expected.to_string());
    }
}
