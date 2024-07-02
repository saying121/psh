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
use profiling::calc::calc_it::Op;
use wasmtime::component::{Linker, ResourceTable};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Calculator;

wasmtime::component::bindgen!({
    path: "../../../psh-sdk-wit/wit/deps/calc",
    world: "imports",
    with: {
        "profiling:calc/calc-it/calculator": Calculator,
    },
    trappable_imports: true,
});

#[derive(Debug, Default)]
pub struct CalcCtx {
    table: ResourceTable,
}

impl profiling::calc::calc_it::Host for CalcCtx {}

impl profiling::calc::calc_it::HostCalculator for CalcCtx {
    // fn calc(&mut self, op: Op, a: u32, b: u32) -> u32 {
    //     match op {
    //         Op::Add => a + b,
    //         Op::Sub => a - b,
    //     }
    // }
    fn calc(&mut self, op: Op, a: u32, b: u32) -> wasmtime::Result<u32> {
        // #[allow(unconditional_panic)]
        // let _a = 1 / 0;
        Ok(match op {
            Op::Add => a + b,
            Op::Sub => a - b,
        })
    }
    fn drop(&mut self, rep: wasmtime::component::Resource<Calculator>) -> wasmtime::Result<()> {
        self.table.delete(rep)?;
        Ok(())
    }
}

pub fn add_to_linker<T>(
    l: &mut Linker<T>,
    f: impl (Fn(&mut T) -> &mut CalcCtx) + Copy + Send + Sync + 'static,
) -> anyhow::Result<()> {
    crate::Imports::add_to_linker(l, f)
}
