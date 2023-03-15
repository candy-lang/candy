use extension_trait::extension_trait;
use std::{
    fmt::{self, Display, Formatter},
    iter::Sum,
    ops::Add,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Indentation(pub usize);
impl Indentation {
    pub fn width(self) -> usize {
        self.0 * 2
    }
    pub fn is_indented(self) -> bool {
        self.0 > 0
    }

    pub fn with_indent(self) -> Self {
        Self(self.0 + 1)
    }
}
impl Display for Indentation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for _ in 0..self.0 {
            write!(f, "  ")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Width {
    Singleline(usize),
    Multiline,
}
impl Width {
    pub const MAX: usize = 100;
    pub const SPACE: Width = Width::Singleline(1);

    fn from_width(width: usize) -> Self {
        Width::from_width_and_max(width, Width::MAX)
    }
    pub fn from_width_and_max(width: usize, max_width: usize) -> Self {
        if width > max_width {
            Width::Multiline
        } else {
            Width::Singleline(width)
        }
    }
    pub fn is_singleline(&self) -> bool {
        match self {
            Width::Singleline(_) => true,
            Width::Multiline => false,
        }
    }
    pub fn is_multiline(&self) -> bool {
        !self.is_singleline()
    }

    pub fn fits(&self, indentation: Indentation) -> bool {
        self.fits_in(Width::MAX - indentation.width())
    }
    pub fn fits_in(&self, max_width: usize) -> bool {
        match self {
            Width::Singleline(width) => width <= &max_width,
            Width::Multiline => false,
        }
    }
}
impl Default for Width {
    fn default() -> Self {
        Width::Singleline(0)
    }
}

macro_rules! width_add {
    (<$($lifetimes:lifetime),*>, $self_type:ty, $other_type:ty) => {
        impl<$($lifetimes),*> Add<$other_type> for $self_type {
            type Output = Width;
            fn add(self, rhs: $other_type) -> Self::Output {
                match (self, rhs) {
                    (Width::Singleline(lhs), Width::Singleline(rhs)) => Width::from_width(lhs + rhs),
                    _ => Width::Multline,
                }
            }
        }
    };
    ($self_type:ty, $other_type:ty) => {
        width_add!(<>, $self_type, $other_type);
    };
}
width_add!(Width, Width);
width_add!(<'a>, Width, &'a Width);
width_add!(<'a>, &'a Width, Width);
width_add!(<'a, 'b>, &'a Width, &'b Width);

impl Sum for Width {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Width::default(), |acc, width| &acc + &width)
    }
}

#[extension_trait]
pub impl StringWidth for str {
    fn width(&self) -> Width {
        if self.contains('\n') {
            Width::Multiline
        } else {
            Width::Singleline(unicode_width::UnicodeWidthStr::width(self))
        }
    }
}
