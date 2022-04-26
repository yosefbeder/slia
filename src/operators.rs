use super::token;

#[derive(Clone, Copy)]
pub enum Associativity {
    Left,
    Right,
}

pub const OPERATORS: [(Option<u8>, Option<u8>, Option<u8>, Option<Associativity>); token::NUMBER] = [
    (None, None, Some(1), None),                         // 0
    (None, None, None, None),                            // 1
    (None, None, None, None),                            // 2
    (None, None, None, None),                            // 3
    (None, None, Some(1), None),                         // 4
    (None, None, None, None),                            // 5
    (None, None, Some(1), None),                         // 6
    (None, Some(4), None, Some(Associativity::Left)),    // 7
    (Some(2), Some(4), None, Some(Associativity::Left)), // 8
    (None, Some(3), None, Some(Associativity::Left)),    // 9
    (None, Some(3), None, Some(Associativity::Left)),    // 10
    (None, Some(3), None, Some(Associativity::Left)),    // 11
    (None, None, None, None),                            // 12
    (None, None, None, None),                            // 13
    (None, None, None, None),                            // 14
    (None, Some(9), None, Some(Associativity::Right)),   // 15
    (None, Some(6), None, Some(Associativity::Left)),    // 16
    (Some(2), None, None, None),                         // 17
    (None, Some(6), None, Some(Associativity::Left)),    // 18
    (None, Some(5), None, Some(Associativity::Left)),    // 19
    (None, Some(5), None, Some(Associativity::Left)),    // 20
    (None, Some(5), None, Some(Associativity::Left)),    // 21
    (None, Some(5), None, Some(Associativity::Left)),    // 22
    (None, Some(7), None, Some(Associativity::Left)),    // 23
    (None, Some(8), None, Some(Associativity::Left)),    // 24
    (None, None, None, None),                            // 25
    (None, None, None, None),                            // 26
    (None, None, None, None),                            // 27
    (None, None, None, None),                            // 28
    (None, None, None, None),                            // 29
    (None, None, None, None),                            // 30
    (None, None, None, None),                            // 31
    (None, None, None, None),                            // 32
    (None, None, None, None),                            // 33
    (None, None, None, None),                            // 34
    (None, None, None, None),                            // 35
    (None, None, None, None),                            // 36
    (None, None, None, None),                            // 37
    (None, None, None, None),                            // 38
    (None, None, None, None),                            // 39
    (None, None, None, None),                            // 40
    (None, None, None, None),                            // 41
    (None, None, None, None),                            // 42
    (None, None, None, None),                            // 43
    (None, None, None, None),                            // 44
    (None, None, None, None),                            // 45
    (None, None, None, None),                            // 46
    (None, None, None, None),                            // 47
];