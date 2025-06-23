use core::mem::MaybeUninit;
use core::mem::ManuallyDrop;
use core::ops::BitAnd;
use core::ops::BitOr;
use core::ops::Not;
use core::ops::Shl;
use core::ops::Shr;
use core::ops::Sub;
use core::ptr::read_volatile;
use core::ptr::write_volatile;
use crate::result::Result;


#[repr(transparent)]
#[derive(Debug)]
pub struct Volatile<T> {
    value: T,
}
impl<T: Default> Default for Volatile<T> {
    fn default() -> Self {
        Self {
            value: T::default()
        }
    }
}
impl<T: Clone> Clone for Volatile<T> {
    fn clone(&self) -> Self {
        let this = MaybeUninit::uninit();
        let mut this: Self = unsafe { this.assume_init()};
        this.write(self.read());
        this
    }
}
impl<T> Volatile<T> {
    pub fn read(&self) -> T {
        unsafe { read_volatile(&self.value) }
    }
    pub fn write(&mut self, new_value: T) {
        unsafe { write_volatile(&mut self.value, new_value) }
    }
}
impl<
    T: Shl<usize, Output = T>
    + Shr<usize, Output = T>
    + BitOr<Output = T>
    + BitAnd<Output = T>
    + Not<Output = T>
    + From<u8>
    + Sub<T, Output = T>
    + PartialEq<T>
    + Copy
> Volatile<T>
{
    pub fn write_bits(
        &mut self,
        shift: usize,
        width: usize,
        value: T
    ) -> Result<()> {
        let mask = (T::from(1) << width) - T::from(1);
        if mask & value != value {
            return Err("Value out of range");
        }
        let mask = mask << shift;
        self.write((value << shift) | (self.read() & !mask));
        Ok(())
    }
    pub fn read_bits(&self, shift: usize, width: usize) -> T {
        let mask = (T::from(1) << width) - T::from(1);
        (self.read() >> shift) & mask
    }
}