use std::fmt::{self, Display};
use std::iter::Iterator;

use failure::{format_err, Error};
use itertools::join;
use rand::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    Assassin,
    Merlin,
    Mordred,
    Morgana,
    Oberon,
    Percival,
    Loyal,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Alliance {
    Resistance,
    Spy,
}

use self::Alliance::*;
use self::Role::*;

impl Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Role {
    pub fn alliance(self) -> Alliance {
        match self {
            Merlin | Percival | Loyal => Resistance,
            Assassin | Mordred | Morgana | Oberon => Spy,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Assassin => "刺客",
            Merlin => "梅林",
            Mordred => "莫德雷德",
            Morgana => "莫甘娜",
            Oberon => "奥伯伦",
            Percival => "派西维尔",
            Loyal => "忠臣",
        }
    }
}

#[derive(Clone, Debug)]
pub enum SeeingBy {
    Normal,
    Spy(Vec<(usize, String)>),
    Merlin(Vec<(usize, String)>, Vec<(usize, String)>),
    Percival(Vec<(usize, String)>),
}

impl SeeingBy {
    pub fn text(&self) -> String {
        self.text_with_formatter(|&(_, ref name)| name.clone())
    }

    pub fn text_from_player(&self, id: usize) -> String {
        self.text_with_formatter(move |&(i, ref name)| {
            if i == id {
                "你".to_owned()
            } else {
                name.clone()
            }
        })
    }

    fn text_with_formatter<F>(&self, f: F) -> String
    where
        F: FnMut(&(usize, String)) -> String + Copy,
    {
        match self {
            SeeingBy::Normal => "".to_owned(),
            SeeingBy::Spy(spies) => format!("{} 都是坏人", join(spies.iter().map(f), "、")),
            SeeingBy::Merlin(resistances, spies) => format!(
                "{} 都是好人\n{} 都是坏人",
                join(resistances.iter().map(f), "、"),
                join(spies.iter().map(f), "、"),
            ),
            SeeingBy::Percival(merlin_list) => format!(
                "{} 当中有一个是梅林，另一个是莫甘娜",
                join(merlin_list.iter().map(f), " 和 "),
            ),
        }
    }
}

pub struct Assignment {
    pub players: Vec<(String, Role)>,
}

impl Assignment {
    pub fn new<T>(names: T) -> Result<Assignment, Error>
    where
        T: Iterator<Item = String>,
    {
        let names_array: Vec<_> = names.collect();
        let roles = deal(names_array.len())?;

        Ok(Assignment {
            players: names_array.into_iter().zip(roles).collect(),
        })
    }

    pub fn player_number(&self) -> usize {
        self.players.len()
    }

    pub fn get_player(&self, index: usize) -> Option<(&str, Role)> {
        self.players
            .get(index)
            .map(|&(ref name, role)| (name.as_ref(), role))
    }

    pub fn see_from_role(&self, role: Role) -> SeeingBy {
        match role {
            Assassin | Morgana | Mordred => {
                SeeingBy::Spy(self.filter_players(|role| role.alliance() == Spy && role != Oberon))
            }
            Merlin => {
                // 梅林看不到莫德雷德
                let resistances =
                    self.filter_players(|role| !(role.alliance() == Spy && role != Mordred));
                let spies = self.filter_players(|role| role.alliance() == Spy && role != Mordred);

                SeeingBy::Merlin(resistances, spies)
            }
            Percival => SeeingBy::Percival(self.filter_players(|role| match role {
                Merlin | Morgana => true,
                _ => false,
            })),
            Oberon | Loyal => SeeingBy::Normal,
        }
    }

    fn filter_players<F>(&self, f: F) -> Vec<(usize, String)>
    where
        F: Fn(Role) -> bool,
    {
        self.players
            .iter()
            .enumerate()
            .filter_map(|(id, &(ref name, role))| {
                if f(role) {
                    Some((id, name.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

pub const LOWER_ROOM_SIZE: usize = 5;
pub const UPPER_ROOM_SIZE: usize = ROLES.len();

const ROLES: &'static [Role] = &[
    Merlin, Assassin, Percival, Morgana, Loyal, Loyal, Oberon, Loyal, Loyal, Mordred,
];

pub fn deal(number: usize) -> Result<Vec<Role>, Error> {
    if number < LOWER_ROOM_SIZE || number > UPPER_ROOM_SIZE {
        return Err(format_err!("invalid player number: {}", number));
    }
    let mut roles = (&ROLES[..number]).to_owned();
    let mut rng = rand::thread_rng();
    roles.shuffle(&mut rng);

    Ok(roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join() {
        let str_list: &[&str] = &["hello", "world"];
        assert_eq!("hello world".to_owned(), join(str_list.iter(), " "));

        assert_eq!(
            "hello world".to_owned(),
            join(vec!["hello".to_owned(), "world".to_owned()].iter(), " "),
        );
    }
}
