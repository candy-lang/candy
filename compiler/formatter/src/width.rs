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
    Multiline { last_line_width: Option<usize> },
}
impl Width {
    pub const MAX: usize = 100;
    pub const SPACE: Width = Width::Singleline(1);

    pub fn multiline() -> Self {
        Width::Multiline {
            last_line_width: None,
        }
    }

    fn from_width(width: usize) -> Self {
        Width::from_width_and_max(width, Width::MAX)
    }
    pub fn from_width_and_max(width: usize, max_width: usize) -> Self {
        if width > max_width {
            Width::multiline()
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

    pub fn fits(&self, indentation: Indentation) -> bool {
        self.fits_in(Width::MAX - indentation.width())
    }
    pub fn fits_in(&self, max_width: usize) -> bool {
        match self {
            Width::Singleline(width) => width <= &max_width,
            Width::Multiline { .. } => false,
        }
    }
    pub fn last_line_fits(&self, indentation: Indentation, extra_width: Width) -> bool {
        let Width::Singleline(extra_width) = extra_width else { return false; };
        match self {
            Width::Singleline(self_width) => {
                indentation.width() + self_width + extra_width <= Width::MAX
            }
            Width::Multiline { last_line_width } => {
                last_line_width.unwrap() + extra_width <= Width::MAX
            }
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
        match (self, rhs) {
            (Width::Singleline(lhs), Width::Singleline(rhs)) => Width::from_width(lhs + rhs),
            (_, Width::Multiline { last_line_width }) => Width::Multiline {
                last_line_width: *last_line_width,
            },
            (
                Width::Multiline {
                    last_line_width: None,
                },
                Width::Singleline(_),
            ) => Width::multiline(),
            (
                Width::Multiline {
                    last_line_width: Some(last_line_width),
                },
                Width::Singleline(width),
            ) => {
                let total_width = last_line_width + width;
                Width::Multiline {
                    last_line_width: if total_width <= Width::MAX {
                        Some(total_width)
                    } else {
                        None
                    },
                }
            }
        }
    }
}

impl AddAssign<Width> for Width {
    fn add_assign(&mut self, rhs: Width) {
        *self = &*self + &rhs;
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
        if let Some(index) = self.rfind('\n') {
            Width::Multiline {
                last_line_width: Some(unicode_width::UnicodeWidthStr::width(&self[index + 1..])),
            }
        } else {
            Width::Singleline(unicode_width::UnicodeWidthStr::width(self))
        }
    }
}
