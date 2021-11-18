//! The CST is a tree representing the structure of source code.

use super::utils::*;

/// Tokens are the leaves of the CST.
mod token {
    use super::Span;

    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Token<T: std::fmt::Debug + PartialEq + Eq + std::hash::Hash> {
        pub span: Span,
        pub value: T,
    }

    pub struct Content {
        pub trailing_trivia: Vec<ContentTrivia>,
    }
    pub enum ContentTrivia {
        Whitespace(Whitespace),
        Comment(Comment),
    }

    mod keyword {
        use std::fmt::{self, Display, Formatter};

        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Use;
        impl Display for Use {
            fn fmt(&self, f: &Formatter<'_>) -> fmt::Result {
                write!(f, "use")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Crate;
        impl Display for Crate {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "use")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Module;
        impl Display for Module {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "module")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Trait;
        impl Display for Trait {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "trait")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Impl;
        impl Display for Impl {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "impl")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Type;
        impl Display for Type {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "type")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Fun;
        impl Display for Fun {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "fun")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Let;
        impl Display for Let {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "let")
            }
        }
        
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Return;
        impl Display for Return {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "ueturnse")
            }
        }
    }


    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Identifier(pub String);
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct IntLiteral(pub u64);
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct StringLiteral(pub String);
    pub enum Operator {
        Minus, // -
        // MinusGreater, // ->
        // Comma, // ,
        // Colon, // :
        ExlamationEqual, // !=
        // Quote, // "
        // OpeningParenthesis(OpeningParenthesis), // (
        // ClosingParenthesis(ClosingParenthesis), // )
        // OpeningBrace(OpeningBrace), // {
        // ClosingBrace(ClosingBrace), // }
        // OpeningBracket(OpeningBracket), // [
        // ClosingBracket(ClosingBracket), // ]
        Star,         // *
        Slash,        // /
        Ampersand,    // &
        Percent,      // %
        Plus,         // +
        Smaller,      // <
        SmallerEqual, // <=
        // Equal, // =
        EqualEqual,   // ==
        EqualGreater, // =>
        Greater,      // >
        GreaterEqual, // >=
        Bar,          // |
        TildeSlash,   // ~/
                      // Dot, // .
                      // Backslash, // \
    }
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct MinusGreater;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Comma;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Colon;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Quote;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct OpeningParenthesis;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct ClosingParenthesis;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct OpeningBrace;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct ClosingBrace;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct OpeningBracket;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct ClosingBracket;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Equal;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Dot;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Backslash;


    #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct Whitespace(pub String);
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct Comment {
        pub content: String,
        pub kind: CommentKind,
    }
    pub enum CommentKind {
        Doc,
        Line,
    }
}
use token::Token;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Id(u64);

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Node<T: std::fmt::Debug + PartialEq + Eq + std::hash::Hash> {
    pub id: Id,
    pub value: T,
}

pub struct Declaration {
    modifiers: Vec<Node<Token<token::Identifier>>>,
    kind: DeclarationKind,
}
pub enum DeclarationKind {
    Module {
        keyword: Node<token::ModuleKeyword>,
        name: Option<Node<IdentifierToken>>,
        content: Option<Node<CstDeclarationContent>>,
    },
    Trait {
        keyword: Node<Token<token::TraitKeyword>>,
        name: Option<Node<Token<token::Identifier>>>,
        typeParameters: Option<Node<CstTypeParameters>>,
        upperBound: Option<(Node<token::Colon>, Option<Node<InlineType>>)>,
        content: Option<Node<CstDeclarationContent>>,
    },
    Impl {
        keyword: Node<Token<token::ImplKeyword>>,
        typeParameters: Option<Node<CstTypeParameters>>,
        type_: Option<Node<InlineType>>,
        traits: Option<(Node<Token<token::Colon>>, Option<Node<InlineType>>)>,
        content: Option<Node<CstDeclarationContent>>,
    },
    Type {
        keyword: Node<Token<token::TypeKeyword>>,
        name: Option<Node<Token<token::Identifier>>>,
        typeParameters: Option<Node<CstTypeParameters>>,
        type_: Option<(Node<Token<token::Colon>>, Option<Node<CstInlineType>>)>,
    },
    Function {
        keyword: Node<Token<token::FunKeyword>>,
        name: Option<Node<Token<token::Identifier>>,
        typeParameters: Option<Node<CstTypeParameters>>,
        valueParameters: Option<Node<CstValueParameters>>,
        returnType: Option<(Node<PunctuationToken>, Option<Node<CstInlineType>>)>,
        body: Option<Node<CstBlockBody>>,
    },
}

// pub modifiers: Vec<CstNode<IdentifierToken>>,
// pub keyword: CstNode<KeywordToken>,

// public trait CstDeclarationWithName {
//   let name: Maybe<Node<IdentifierToken>>
// }

// public class CstModule {

// }
// impl CstModule: CstDeclaration & CstDeclarationWithName

// public class CstTrait {
//   public let modifiers: List<CstNode<IdentifierToken>>
//   public let keyword: CstNode<KeywordToken>
//   public let name: Maybe<CstNode<IdentifierToken>>
//   public let typeParameters: Maybe<CstNode<CstTypeParameters>>
//   public let upperBound: Maybe<(CstNode<PunctuationToken>, Maybe<CstNode<CstInlineType>>)>
//   public let content: Maybe<CstNode<CstDeclarationContent>>
// }
// impl CstTrait: CstDeclaration & CstDeclarationWithName

// public class CstImpl {
//   public let modifiers: List<CstNode<IdentifierToken>>
//   public let keyword: CstNode<KeywordToken>
//   public let typeParameters: Maybe<CstNode<CstTypeParameters>>
//   public let type: Maybe<CstNode<CstInlineType>>
//   public let traits: Maybe<(CstNode<PunctuationToken>, Maybe<CstNode<CstInlineType>>)>
//   public let content: Maybe<CstNode<CstDeclarationContent>>
// }
// impl CstImpl: CstDeclaration

// public class CstType {
//   public let modifiers: List<CstNode<IdentifierToken>>
//   public let keyword: CstNode<KeywordToken>
//   public let name: Maybe<CstNode<IdentifierToken>>
//   public let typeParameters: Maybe<CstNode<CstTypeParameters>>
//   public let type: Maybe<(CstNode<PunctuationToken>, Maybe<CstNode<CstInlineType>>)>
// }
// impl CstType: CstDeclaration & CstDeclarationWithName

// public class CstFunction {
//   public let modifiers: List<CstNode<IdentifierToken>>
//   public let keyword: CstNode<KeywordToken>
//   public let name: Maybe<CstNode<IdentifierToken>>
//   public let typeParameters: Maybe<CstNode<CstTypeParameters>>
//   public let valueParameters: Maybe<CstNode<CstValueParameters>>
//   public let returnType: Maybe<(CstNode<PunctuationToken>, Maybe<CstNode<CstInlineType>>)>
//   public let body: Maybe<CstNode<CstBlockBody | CstExpressionBody>>
// }
// impl CstFunction: CstDeclaration & CstDeclarationWithName

// public class CstValueParameters {
//   public let openingParenthesis: CstNode<PunctuationToken>
//   public let valueParameters: List<CstNode<CstValueParameter | PunctuationToken>>
//   public let closingParenthesis: Maybe<CstNode<PunctuationToken>>
// }
// public class CstValueParameter {
//   public let modifiers: List<CstNode<IdentifierToken>>
//   public let name: Maybe<CstNode<IdentifierToken>>
//   public let type: Maybe<(CstNode<PunctuationToken>, Maybe<CstNode<CstInlineType>>)>
//   public let defaultValue: Maybe<CstNode<CstExpressionBody>>
// }

// public class CstDeclarationContent {
//   public let openingCurlyBrace: CstNode<PunctuationToken>
//   public let innerDeclarations: List<CstNode<CstDeclaration>>
//   public let closingCurlyBrace: Maybe<CstNode<PunctuationToken>>
// }

// public class CstBlockBody {
//   public let openingCurlyBrace: CstNode<PunctuationToken>
//   public let expressions: List<CstNode<CstExpression>>
//   public let closingCurlyBrace: Maybe<CstNode<PunctuationToken>>
// }
// public class CstExpressionBody {
//   public let equalsSign: CstNode<PunctuationToken>
//   public let expression: Maybe<CstNode<CstExpression>>
// }
