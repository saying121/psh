// Copyright (c) 2023-2024 Optimatist Technology Co., Ltd. All rights reserved.
// DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
//
// This file is part of PSH.
//
// PSH is free software: you can redistribute it and/or modify it under the terms of the GNU Lesser General Public License
// as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
//
// PSH is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
// the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License along with Performance Savior Home (PSH). If not,
// see <https://www.gnu.org/licenses/>.
use std::{
    env,
    path::{Path, PathBuf},
};

use nix::unistd::geteuid;

#[allow(dead_code)]
fn check_root_privilege() -> bool {
    let euid = geteuid();
    euid.is_root()
}

#[allow(dead_code)]
pub(crate) fn which<P>(exe_name: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let full_path = dir.join(&exe_name);
            if full_path.is_file() {
                Some(full_path)
            } else {
                None
            }
        })
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_check_root_privilege() {
        use super::check_root_privilege;
        // Test when the user has root privilege
        assert_eq!(check_root_privilege(), true);

        // Test when the user does not have root privilege
        // You can modify this test case to simulate a non-root user
        // by returning a non-root euid from geteuid() function
        // assert_eq!(check_root_privilege(), false);
    }

    #[test]
    fn test_which() {
        use super::which;
        println!("{:?}", which("ls").unwrap());
    }
}
