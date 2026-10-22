// Output side: diversion 0 is standard output, positive diversions are
// held in memory, negative diversions discard text.

use crate::m4::M4;

impl M4 {
    pub fn output_text(&mut self, text: &[u8]) {
        let d = self.current_diversion;
        if d < 0 {
            return;
        }
        if d == 0 {
            self.stdout_buf.extend_from_slice(text);
            if self.stdout_buf.len() > (1 << 16) {
                self.flush_stdout();
            }
        } else {
            self.diversions.entry(d).or_default().extend_from_slice(text);
        }
    }

    pub fn make_diversion(&mut self, divnum: i32) {
        self.current_diversion = divnum;
    }

    pub fn insert_diversion(&mut self, divnum: i32) {
        if divnum <= 0 || self.current_diversion == divnum {
            return;
        }
        if let Some(data) = self.diversions.remove(&divnum) {
            self.output_text(&data);
        }
    }

    pub fn undivert_all(&mut self) {
        let keys: Vec<i32> = self.diversions.keys().cloned().collect();
        for k in keys {
            if k != self.current_diversion {
                if let Some(data) = self.diversions.remove(&k) {
                    self.output_text(&data);
                }
            }
        }
    }
}
