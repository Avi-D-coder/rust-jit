// `Step` impls of the integer primitives (real core: iter/range.rs).
// Arithmetic stays within each type's range; bounds are written literally.
use crate::iter::Step;
use crate::option::Option;

impl Step for i8 {
    fn steps_between(start: &i8, end: &i8) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end as i64 - *start as i64) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: i8, count: usize) -> Option<i8> {
        if count as u64 > (127_i64 - start as i64) as u64 {
            Option::None
        } else {
            Option::Some((start as i64 + count as i64) as i8)
        }
    }
    fn backward_checked(start: i8, count: usize) -> Option<i8> {
        if count as u64 > (start as i64 - (-128_i64)) as u64 {
            Option::None
        } else {
            Option::Some((start as i64 - count as i64) as i8)
        }
    }
}
impl Step for i16 {
    fn steps_between(start: &i16, end: &i16) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end as i64 - *start as i64) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: i16, count: usize) -> Option<i16> {
        if count as u64 > (32767_i64 - start as i64) as u64 {
            Option::None
        } else {
            Option::Some((start as i64 + count as i64) as i16)
        }
    }
    fn backward_checked(start: i16, count: usize) -> Option<i16> {
        if count as u64 > (start as i64 - (-32768_i64)) as u64 {
            Option::None
        } else {
            Option::Some((start as i64 - count as i64) as i16)
        }
    }
}
impl Step for i32 {
    fn steps_between(start: &i32, end: &i32) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end as i64 - *start as i64) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: i32, count: usize) -> Option<i32> {
        if count as u64 > (2147483647_i64 - start as i64) as u64 {
            Option::None
        } else {
            Option::Some((start as i64 + count as i64) as i32)
        }
    }
    fn backward_checked(start: i32, count: usize) -> Option<i32> {
        if count as u64 > (start as i64 - (-2147483648_i64)) as u64 {
            Option::None
        } else {
            Option::Some((start as i64 - count as i64) as i32)
        }
    }
}
impl Step for i64 {
    fn steps_between(start: &i64, end: &i64) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = if *start >= 0 || *end < 0 {
                (*end - *start) as usize
            } else {
                (*end as usize) + ((-(*start + 1)) as usize) + 1
            };
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: i64, count: usize) -> Option<i64> {
        let room = if start >= 0 {
            (9223372036854775807_i64 - start) as usize
        } else {
            9223372036854775807_usize + ((-(start + 1)) as usize) + 1
        };
        if count > room {
            Option::None
        } else if count <= 9223372036854775807_usize {
            Option::Some(start + count as i64)
        } else {
            Option::Some(start + 9223372036854775807_i64 + (count - 9223372036854775807_usize) as i64)
        }
    }
    fn backward_checked(start: i64, count: usize) -> Option<i64> {
        let room = if start < 0 {
            ((start - (-9223372036854775808_i64 + 1)) as usize) + 1
        } else {
            (start as usize) + 9223372036854775807_usize + 1
        };
        if count > room {
            Option::None
        } else if count <= 9223372036854775807_usize {
            Option::Some(start - count as i64)
        } else {
            Option::Some(start - 9223372036854775807_i64 - (count - 9223372036854775807_usize) as i64)
        }
    }
}
impl Step for isize {
    fn steps_between(start: &isize, end: &isize) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = if *start >= 0 || *end < 0 {
                (*end - *start) as usize
            } else {
                (*end as usize) + ((-(*start + 1)) as usize) + 1
            };
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: isize, count: usize) -> Option<isize> {
        let room = if start >= 0 {
            (9223372036854775807_isize - start) as usize
        } else {
            9223372036854775807_usize + ((-(start + 1)) as usize) + 1
        };
        if count > room {
            Option::None
        } else if count <= 9223372036854775807_usize {
            Option::Some(start + count as isize)
        } else {
            Option::Some(start + 9223372036854775807_isize + (count - 9223372036854775807_usize) as isize)
        }
    }
    fn backward_checked(start: isize, count: usize) -> Option<isize> {
        let room = if start < 0 {
            ((start - (-9223372036854775808_isize + 1)) as usize) + 1
        } else {
            (start as usize) + 9223372036854775807_usize + 1
        };
        if count > room {
            Option::None
        } else if count <= 9223372036854775807_usize {
            Option::Some(start - count as isize)
        } else {
            Option::Some(start - 9223372036854775807_isize - (count - 9223372036854775807_usize) as isize)
        }
    }
}
impl Step for u8 {
    fn steps_between(start: &u8, end: &u8) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end - *start) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: u8, count: usize) -> Option<u8> {
        let room = (255_u8 - start) as u64;
        if count as u64 > room {
            Option::None
        } else {
            Option::Some(start + count as u8)
        }
    }
    fn backward_checked(start: u8, count: usize) -> Option<u8> {
        if count as u64 > start as u64 {
            Option::None
        } else {
            Option::Some(start - count as u8)
        }
    }
}
impl Step for u16 {
    fn steps_between(start: &u16, end: &u16) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end - *start) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: u16, count: usize) -> Option<u16> {
        let room = (65535_u16 - start) as u64;
        if count as u64 > room {
            Option::None
        } else {
            Option::Some(start + count as u16)
        }
    }
    fn backward_checked(start: u16, count: usize) -> Option<u16> {
        if count as u64 > start as u64 {
            Option::None
        } else {
            Option::Some(start - count as u16)
        }
    }
}
impl Step for u32 {
    fn steps_between(start: &u32, end: &u32) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end - *start) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: u32, count: usize) -> Option<u32> {
        let room = (4294967295_u32 - start) as u64;
        if count as u64 > room {
            Option::None
        } else {
            Option::Some(start + count as u32)
        }
    }
    fn backward_checked(start: u32, count: usize) -> Option<u32> {
        if count as u64 > start as u64 {
            Option::None
        } else {
            Option::Some(start - count as u32)
        }
    }
}
impl Step for u64 {
    fn steps_between(start: &u64, end: &u64) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end - *start) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: u64, count: usize) -> Option<u64> {
        let room = (18446744073709551615_u64 - start) as u64;
        if count as u64 > room {
            Option::None
        } else {
            Option::Some(start + count as u64)
        }
    }
    fn backward_checked(start: u64, count: usize) -> Option<u64> {
        if count as u64 > start as u64 {
            Option::None
        } else {
            Option::Some(start - count as u64)
        }
    }
}
impl Step for usize {
    fn steps_between(start: &usize, end: &usize) -> (usize, Option<usize>) {
        if *start <= *end {
            let steps = (*end - *start) as usize;
            (steps, Option::Some(steps))
        } else {
            (0, Option::None)
        }
    }
    fn forward_checked(start: usize, count: usize) -> Option<usize> {
        let room = (18446744073709551615_usize - start) as u64;
        if count as u64 > room {
            Option::None
        } else {
            Option::Some(start + count as usize)
        }
    }
    fn backward_checked(start: usize, count: usize) -> Option<usize> {
        if count as u64 > start as u64 {
            Option::None
        } else {
            Option::Some(start - count as usize)
        }
    }
}
