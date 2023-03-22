use extension_trait::extension_trait;
use std::{
    fmt::{self, Display, Formatter},
    iter::Sum,
    ops::{Add, AddAssign},
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
    Multiline {
        /// Only [Some] if the expression can be used as a trailing multiline expression, e.g., a
        /// trailing lambda.
        first_line_width: Option<usize>,
        last_line_width: Option<usize>,
    },
}
impl Width {
    pub const MAX: usize = 100;
    pub const SPACE: Width = Width::Singleline(1);

    pub fn multiline(
        first_line_width: impl Into<Option<usize>>,
        last_line_width: impl Into<Option<usize>>,
    ) -> Self {
        Width::Multiline {
            first_line_width: first_line_width.into(),
            last_line_width: last_line_width.into(),
        }
    }

    fn from_width(width: usize) -> Self {
        Width::from_width_and_max(width, Width::MAX)
    }
    pub fn from_width_and_max(width: usize, max_width: usize) -> Self {
        if width > max_width {
            Width::multiline(None, None)
        } else {
            Width::Singleline(width)
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Width::Singleline(width) => *width == 0,
            Width::Multiline { .. } => false,
        }
    }
    pub fn is_singleline(&self) -> bool {
        match self {
            Width::Singleline(_) => true,
            Width::Multiline { .. } => false,
        }
    }
    pub fn is_multiline(&self) -> bool {
        !self.is_singleline()
    }
    pub fn singleline_width(&self) -> Option<usize> {
        match self {
            Width::Singleline(width) => Some(*width),
            Width::Multiline { .. } => None,
        }
    }
    pub fn first_line_width(&self) -> Option<Width> {
        match self {
            Width::Singleline(width) => Some(Width::Singleline(*width)),
            Width::Multiline {
                first_line_width, ..
            } => first_line_width.map(Width::Singleline),
        }
    }
    pub fn without_first_line_width(&self) -> Width {
        match self {
            Width::Singleline(width) => Width::Singleline(*width),
            Width::Multiline {
                last_line_width, ..
            } => Width::Multiline {
                first_line_width: None,
                last_line_width: *last_line_width,
            },
        }
    }

    pub fn fits(&self, indentation: Indentation) -> bool {
        self.fits_in(Width::MAX - indentation.width())
    }
    pub fn fits_in(&self, max_width: usize) -> bool {
        match self {
            Width::Singleline(width) => width <= &max_width,
            Width::Multiline { .. } => false,
        }
    }
    pub fn last_line_fits(&self, indentation: Indentation, extra_width: &Width) -> bool {
        let Width::Singleline(extra_width) = extra_width else { return false; };
        match self {
            Width::Singleline(self_width) => {
                indentation.width() + self_width + extra_width <= Width::MAX
            }
            Width::Multiline {
                last_line_width, ..
            } => last_line_width.unwrap() + extra_width <= Width::MAX,
        }
    }
}
impl Default for Width {
    fn default() -> Self {
        Width::Singleline(0)
    }
}

impl Add<Width> for Width {
    type Output = Width;
    fn add(self, rhs: Width) -> Self::Output {
        &self + &rhs
    }
}
impl<'a> Add<Width> for &'a Width {
    type Output = Width;
    fn add(self, rhs: Width) -> Self::Output {
        self + &rhs
    }
}
impl<'a> Add<&'a Width> for Width {
    type Output = Width;
    fn add(self, rhs: &'a Width) -> Self::Output {
        &self + rhs
    }
}
impl<'a, 'b> Add<&'b Width> for &'a Width {
    type Output = Width;
    fn add(self, rhs: &'b Width) -> Self::Output {
        fn add_singleline(
            lhs: impl Into<Option<usize>>,
            rhs: impl Into<Option<usize>>,
        ) -> Option<usize> {
            let (Some(lhs), Some(rhs)) = (lhs.into(), rhs.into()) else { return None; };
            let sum = lhs + rhs;
            if sum <= Width::MAX {
                Some(sum)
            } else {
                None
            }
        }

        match (self, rhs) {
            (Width::Singleline(lhs), Width::Singleline(rhs)) => Width::from_width(lhs + rhs),
            (
                Width::Singleline(lhs),
                Width::Multiline {
                    first_line_width,
                    last_line_width,
                },
            ) => Width::multiline(add_singleline(*lhs, *first_line_width), *last_line_width),
            (
                Width::Multiline {
                    first_line_width,
                    last_line_width,
                },
                Width::Singleline(rhs),
            ) => Width::multiline(*first_line_width, add_singleline(*last_line_width, *rhs)),
            (
                Width::Multiline {
                    first_line_width, ..
                },
                Width::Multiline {
                    last_line_width, ..
                },
            ) => Width::multiline(*first_line_width, *last_line_width),
        }
    }
}

impl AddAssign<Width> for Width {
    fn add_assign(&mut self, rhs: Width) {
        *self += &rhs;
    }
}
impl AddAssign<&Width> for Width {
    fn add_assign(&mut self, rhs: &Width) {
        *self = &*self + rhs;
    }
}

impl Sum for Width {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Width::default(), |acc, width| &acc + &width)
    }
}

#[extension_trait]
pub impl StringWidth for str {
    fn width(&self) -> Width {
        if let Some(first_index) = self.find('\n') {
            let last_index = self.rfind('\n').unwrap();
            Width::multiline(
                unicode_width::UnicodeWidthStr::width(&self[..first_index]),
                unicode_width::UnicodeWidthStr::width(&self[last_index + 1..]),
            )
        } else {
            Width::Singleline(unicode_width::UnicodeWidthStr::width(self))
        }
    }
}
