use super::token::Token;
use std::rc::Rc;

#[derive(Debug)]
pub enum Literal {
    Number(Rc<Token>),
    String(Rc<Token>),
    Bool(Rc<Token>),
    Nil(Rc<Token>),
    List(Vec<Expr>),
    Object(Vec<(Rc<Token>, Option<Expr>)>),
}

#[derive(Debug)]
pub enum Expr {
    Variable(Rc<Token>),
    Literal(Literal),
    Unary(Rc<Token>, Box<Expr>),
    Binary(Rc<Token>, Box<Expr>, Box<Expr>),
    Call(Rc<Token>, Box<Expr>, Vec<Expr>),
    Get(Rc<Token>, Box<Expr>, Box<Expr>),
    Set(Rc<Token>, Box<Expr>, Box<Expr>, Box<Expr>),
    Lambda(Rc<Token>, Vec<Rc<Token>>, Box<Stml>),
}

impl Expr {
    pub fn as_variable(&self) -> Rc<Token> {
        match self {
            Self::Variable(token) => Rc::clone(token),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum Stml {
    Block(Vec<Stml>),
    FunctionDecl(Rc<Token>, Vec<Rc<Token>>, Box<Stml>),
    VarDecl(Rc<Token>, Rc<Token>, Option<Expr>),
    Return(Rc<Token>, Option<Expr>),
    Throw(Rc<Token>, Option<Expr>),
    TryCatch(Box<Stml>, Rc<Token>, Box<Stml>),
    IfElse(Expr, Box<Stml>, Vec<(Expr, Stml)>, Option<Box<Stml>>),
    While(Expr, Box<Stml>),
    Loop(Box<Stml>),
    Break(Rc<Token>),
    Continue(Rc<Token>),
    Import(Rc<Token>, Rc<Token>),
    Export(Rc<Token>, Box<Stml>),
    ForIn(Rc<Token>, Rc<Token>, Expr, Box<Stml>),
    Expr(Expr),
}

impl Stml {
    pub fn as_block(&self) -> &Vec<Stml> {
        match self {
            Self::Block(decls) => decls,
            _ => unreachable!(),
        }
    }
}
