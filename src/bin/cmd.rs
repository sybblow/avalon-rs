use std::io::prelude::*;
use std::iter::Iterator;

use avalon_rs::game::*;

fn main() {
    let stdin = std::io::stdin();
    let names = stdin.lock().lines().filter_map(Result::ok);
    let assignment = Assignment::new(names).unwrap();
    println!("{}", assignment.see_from_role(Role::Merlin).text());
    println!("# ===================================== #");
    println!();
    println!();

    for i in 0..assignment.player_number() {
        if let Some((name, role)) = assignment.get_player(i) {
            print!("{} 的身份是【{}】，", name, role);
            let assignment_text = assignment.see_from_role(role).text_from_player(i);
            if assignment_text.is_empty() {
                println!("没有提示");
            } else {
                println!("看到的提示如下：");
                println!("{}", assignment_text);
            }
            println!("# ===================================== #");
        }
    }
}
