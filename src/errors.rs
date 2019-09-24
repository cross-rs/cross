#![allow(unused_doc_comments)]

use error_chain::error_chain;

error_chain! {
  foreign_links {
    Io(std::io::Error);
  }
}
