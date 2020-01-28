use avalon_rs::game::*;

fn main() {
    for role in deal(7).unwrap() {
        println!("{}: {:?}", role, role.alliance())
    }
}
