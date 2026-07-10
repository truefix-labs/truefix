use crate::error::{FutuError, FutuResult};

pub(crate) fn ensure_ok(ret_type: i32, ret_msg: Option<String>) -> FutuResult<()> {
    if ret_type == 0 {
        Ok(())
    } else {
        Err(FutuError::OpenDError { ret_type, ret_msg })
    }
}

pub(crate) trait ResponseWithS2C {
    type S2c;
    fn ret_type(&self) -> i32;
    fn ret_msg(&self) -> Option<String>;
    fn s2c(self) -> Option<Self::S2c>;
}

macro_rules! impl_response_with_s2c {
    ($ty:ty, $s2c:ty) => {
        impl ResponseWithS2C for $ty {
            type S2c = $s2c;

            fn ret_type(&self) -> i32 {
                self.ret_type
            }

            fn ret_msg(&self) -> Option<String> {
                self.ret_msg.clone()
            }

            fn s2c(self) -> Option<Self::S2c> {
                self.s2c
            }
        }
    };
}

pub(crate) use impl_response_with_s2c;
