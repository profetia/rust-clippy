#![warn(clippy::string_to_string)]
#![allow(clippy::redundant_clone)]

fn main() {
    let mut message = String::from("Hello");
    let mut v = message.to_string();
    //~^ string_to_string
}

mod issue14175 {
    fn string_behind_ref() {
        let mut v1 = String::from("Hello");
        let mut v2 = Some(&v1).map(|s| s.to_string());
        //~^ string_to_string
    }
}
