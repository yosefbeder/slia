//TODO prevent the VM from changing it's state on runtime errors
use super::{
    chunk::{Chunk, Instruction},
    qatam,
    reporter::{Phase, Report, Reporter},
    token::Token,
    utils::combine,
    value::{Arity, Closure, Function, NFunction, UpValue, Value},
};
use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc};

pub struct Frame {
    closure: Rc<Closure>,
    ip: usize,
    slots_start: usize,
}

impl Frame {
    fn new(closure: Rc<Closure>, slots_start: usize) -> Self {
        Frame {
            closure,
            ip: 0,
            slots_start,
        }
    }

    fn get_up_value(&self, idx: usize) -> Rc<RefCell<UpValue>> {
        return Rc::clone(self.closure.up_values.get(idx).unwrap());
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "closure: {}, slots_start: {}",
            Value::Closure(Rc::clone(&self.closure)),
            self.slots_start
        )
    }
}

pub struct Vm {
    //TODO make the stack static!
    stack: Vec<Value>,
    frames: Vec<Frame>,
    globals: HashMap<String, Value>,
    open_up_values: Vec<Rc<RefCell<UpValue>>>,
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: HashMap::new(),
            open_up_values: Vec::new(),
        };

        vm.globals.insert(
            "كنص".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::as_string, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "كصحيح".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::as_int, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "كعشري".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::as_float, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "نوع".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::typ, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "حجم".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::size, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "خصائص".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::properties, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "إدفع".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::push, Arity::Fixed(2)))),
        );
        vm.globals.insert(
            "إسحب".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::pop, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "الوقت".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::time, Arity::Fixed(0)))),
        );
        vm.globals.insert(
            "أغلق".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::exit, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "عشوائي".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::random, Arity::Fixed(0)))),
        );
        vm.globals.insert(
            "إقرأ".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::read, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "إكتب".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::write, Arity::Fixed(2)))),
        );
        vm.globals.insert(
            "جا".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::sin, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "جتا".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::cos, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "ظا".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::tan, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "قتا".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::csc, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "قا".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::sec, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "ظتا".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::cot, Arity::Fixed(1)))),
        );
        vm.globals.insert(
            "إطبع".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::print, Arity::Variadic(1)))),
        );
        vm.globals.insert(
            "إمسح".to_string(),
            Value::NFunction(Rc::new(NFunction::new(qatam::scan, Arity::Fixed(0)))),
        );

        vm
    }

    fn error(&mut self, msg: &str, reporter: &mut dyn Reporter) {
        self.error_at(self.get_cur_token(), msg, reporter);
    }

    fn error_at(&mut self, token: Rc<Token>, msg: &str, reporter: &mut dyn Reporter) {
        let report = Report::new(Phase::Runtime, msg.to_string(), token);
        reporter.error(report);
    }

    //>> Stack manipulation
    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    fn last(&self) -> Value {
        self.stack.last().unwrap().clone()
    }

    fn get(&self, idx: usize) -> Value {
        self.stack.get(idx).unwrap().clone()
    }
    //<<

    //>> Frame manipulation
    fn last_frame(&self) -> &Frame {
        self.frames.last().unwrap()
    }

    fn last_frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    fn get_byte(&self, offset: usize) -> Option<u8> {
        self.frames
            .last()
            .unwrap()
            .closure
            .function
            .chunk
            .get_byte(offset)
    }

    fn get_constant(&self, idx: usize) -> Value {
        self.frames
            .last()
            .unwrap()
            .closure
            .function
            .chunk
            .get_constant(idx)
    }

    fn get_ip(&self) -> usize {
        self.last_frame().ip
    }

    fn get_slots_start(&self) -> usize {
        self.last_frame().slots_start
    }

    fn get_cur_chunk(&self) -> &Chunk {
        &self.last_frame().closure.function.chunk
    }

    fn get_cur_token(&self) -> Rc<Token> {
        self.get_cur_chunk().get_token(self.get_ip())
    }

    fn read_byte_oper(&self) -> u8 {
        self.get_byte(self.get_ip() + 1).unwrap()
    }

    fn read_bytes_oper(&self) -> u16 {
        combine(
            self.get_byte(self.get_ip() + 1).unwrap(),
            self.get_byte(self.get_ip() + 2).unwrap(),
        )
    }

    fn call(&mut self, argc: usize) -> Result<(), String> {
        let idx = self.stack.len() - argc - 1;

        match self.get(idx).clone() {
            Value::Closure(closure) => {
                match closure.function.arity {
                    Arity::Fixed(arity) => {
                        if argc != arity as usize {
                            return Err("تقبل هذه المهمة {arity} معطى".to_string());
                        }
                    }
                    _ => unimplemented!(),
                }

                let frame = Frame::new(Rc::clone(&closure), idx);

                if cfg!(feature = "debug-execution") {
                    println!("[DEBUG] called {frame:?}");
                }

                self.frames.push(frame);
                Ok(())
            }
            Value::NFunction(n_function) => {
                let args = self.stack.split_off(idx);

                match n_function.arity {
                    Arity::Fixed(arity) => {
                        if argc != arity as usize {
                            return Err("تقبل هذه المهمة {arity} معطى".to_string());
                        }
                    }
                    Arity::Variadic(arity) => {
                        if argc < arity as usize {
                            return Err(format!("تقبل هذه المهمة {arity} معطى على الأقل"));
                        }
                    }
                }

                match (n_function.function)(args) {
                    Ok(returned) => {
                        self.push(returned);
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };

                Ok(())
            }
            _ => {
                return Err("يمكن فقط استدعاء الدوال".to_string());
            }
        }
    }

    pub fn call_function(&mut self, function: Function) -> Result<(), String> {
        self.push(Value::Closure(Rc::new(Closure::new(
            Rc::new(function),
            Vec::new(),
        ))));
        self.call(0)
    }
    //<<

    fn close_up_values(&mut self, location: usize) {
        let mut new = Vec::new();

        for up_value in self.open_up_values.iter() {
            let idx;

            match &*up_value.borrow() {
                UpValue::Open(idx_) => idx = *idx_,
                UpValue::Closed(_) => unreachable!(),
            }

            if idx >= location {
                up_value
                    .borrow_mut()
                    .close(self.stack.get(idx).unwrap().clone());
            } else {
                new.push(up_value.clone());
            }
        }
        self.open_up_values = new;
    }

    fn execute_instr(
        &mut self,
        instr: Instruction,
        reporter: &mut dyn Reporter,
    ) -> Result<usize, ()> {
        match instr {
            Instruction::Pop => {
                self.pop();
            }
            Instruction::Constant8 => {
                let idx = self.read_byte_oper() as usize;
                self.push(self.get_constant(idx));
                return Ok(2);
            }
            Instruction::Constant16 => {
                let idx = self.read_bytes_oper() as usize;
                self.push(self.get_constant(idx));
                return Ok(3);
            }
            Instruction::Negate => {
                let popped = self.pop();

                if !popped.is_number() {
                    self.error("يجب أن يكون المعامل رقماً", reporter);
                    return Err(());
                }

                self.push(-popped);
            }
            Instruction::Add => {
                let b = self.pop();
                let a = self.pop();

                self.push(a + b);
            }
            Instruction::Subtract => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_subtractable(&a, &b) {
                    self.error("لا يقبل المعاملان الطرح من بعضهما", reporter);
                    return Err(());
                }

                self.push(a - b);
            }
            Instruction::Multiply => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_multipliable(&a, &b) {
                    self.error("لا يقبل المعاملان الضرب في بعضهما", reporter);
                    return Err(());
                }

                self.push(a * b);
            }
            Instruction::Divide => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_dividable(&a, &b) {
                    self.error("لا يقبل المعاملان القسمة على بعضهما", reporter);
                    return Err(());
                }

                self.push(a / b);
            }
            Instruction::Remainder => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_remainderable(&a, &b) {
                    self.error("لا يقبل المعاملان القسمة على بعضهما", reporter);
                    return Err(());
                }

                self.push(a % b);
            }
            Instruction::Not => {
                let popped = self.pop();
                self.push(!popped);
            }
            Instruction::Equal => {
                let b = self.pop();
                let a = self.pop();
                self.push(Value::Bool(a == b));
            }
            Instruction::Greater => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_numbers(&a, &b) {
                    self.error("يجب أن يكون المعاملان أرقاماً", reporter);
                    return Err(());
                }

                self.push(Value::Bool(a > b));
            }
            Instruction::GreaterEqual => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_numbers(&a, &b) {
                    self.error("يجب أن يكون المعاملان أرقاماً", reporter);
                    return Err(());
                }

                self.push(Value::Bool(a >= b));
            }
            Instruction::Less => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_numbers(&a, &b) {
                    self.error("يجب أن يكون المعاملان أرقاماً", reporter);
                    return Err(());
                }
                self.push(Value::Bool(a < b));
            }
            Instruction::LessEqual => {
                let b = self.pop();
                let a = self.pop();

                if !Value::are_numbers(&a, &b) {
                    self.error("يجب أن يكون المعاملان أرقاماً", reporter);
                    return Err(());
                }

                self.push(Value::Bool(a <= b));
            }
            Instruction::Jump => {
                return Ok(self.read_bytes_oper() as usize);
            }
            Instruction::JumpIfFalse => {
                if self.stack.last().unwrap().is_truthy() {
                    return Ok(3);
                }
                return Ok(self.read_bytes_oper() as usize);
            }
            Instruction::JumpIfTrue => {
                if !self.stack.last().unwrap().is_truthy() {
                    return Ok(3);
                }
                return Ok(self.read_bytes_oper() as usize);
            }
            Instruction::Loop => {
                self.last_frame_mut().ip -= self.read_bytes_oper() as usize;
                return Ok(0);
            }
            Instruction::DefineGlobal => {
                let name = self.pop().to_string();
                let value = self.pop();

                if self.globals.contains_key(&name) {
                    self.error("يوجد متغير بهذا الاسم", reporter);
                    return Err(());
                }

                self.globals.insert(name.clone(), value);
            }
            Instruction::SetGlobal => {
                let name = self.pop().to_string();
                let value = self.last();

                if !self.globals.contains_key(&name) {
                    self.error("لا يوجد متغير بهذا الاسم", reporter);
                    return Err(());
                }

                self.globals.insert(name, value);
            }
            Instruction::GetGlobal => {
                let name = self.pop().to_string();

                if !self.globals.contains_key(&name) {
                    self.error("لا يوجد متغير بهذا الاسم", reporter);
                    return Err(());
                }

                self.push(self.globals.get(&name).unwrap().clone());
            }
            Instruction::GetLocal => {
                let idx = self.get_slots_start() + self.read_byte_oper() as usize;
                self.push(self.get(idx));
                return Ok(2);
            }
            Instruction::SetLocal => {
                let idx = self.get_slots_start() + self.read_byte_oper() as usize;
                *self.stack.get_mut(idx).unwrap() = self.last();
                return Ok(2);
            }
            Instruction::BuildList => {
                let size = self.read_byte_oper() as usize;
                let items = RefCell::new(Vec::new());

                for item in self.stack.drain(self.stack.len() - size..) {
                    items.borrow_mut().push(item);
                }

                self.push(Value::List(Rc::new(items)));
                return Ok(2);
            }
            Instruction::BuildObject => {
                let size = self.read_byte_oper();
                let items = RefCell::new(HashMap::new());

                for _ in 0..size {
                    let value = self.pop();
                    let name = self.pop().to_string();
                    items.borrow_mut().insert(name, value);
                }

                self.push(Value::Object(Rc::new(items)));
                return Ok(2);
            }
            Instruction::Get => {
                let key = self.pop();
                let popped = self.pop();

                match &popped {
                    Value::Object(items) => {
                        if !key.is_string() {
                            self.error("يجب أن يكون اسم الخاصية نصاً", reporter);
                            return Err(());
                        }

                        if let Some(value) = items.borrow().get(&key.to_string()) {
                            self.push(value.clone());
                            return Ok(2);
                        }

                        self.error("لا توجد خاصية بهذا الاسم", reporter);
                        return Err(());
                    }
                    Value::List(items) => {
                        let idx: isize = match key.try_into() {
                            Ok(idx) => idx,
                            Err(_) => {
                                self.error("يجب أن يكون رقم العنصر عدداً صحيحاً", reporter);
                                return Err(());
                            }
                        };

                        if idx >= 0 {
                            match items.borrow().get(idx as usize) {
                                Some(value) => {
                                    self.push(value.clone());
                                    return Ok(1);
                                }
                                None => {
                                    self.error("لا يوجد عنصر بهذا الرقم", reporter);
                                    return Err(());
                                }
                            }
                        } else {
                            match items.borrow().iter().nth_back((idx + 1).abs() as usize) {
                                Some(value) => {
                                    self.push(value.clone());
                                    return Ok(1);
                                }
                                None => {
                                    self.error("لا يوجد عنصر بهذا الرقم", reporter);
                                    return Err(());
                                }
                            }
                        }
                    }
                    Value::String(string) => {
                        let idx: isize = match key.try_into() {
                            Ok(idx) => idx,
                            Err(_) => {
                                self.error("يجب أن يكون رقم العنصر عدداً صحيحاً", reporter);
                                return Err(());
                            }
                        };

                        if idx >= 0 {
                            match string.chars().nth(idx as usize) {
                                Some(value) => {
                                    self.push(Value::String(format!("{value}")));
                                    return Ok(1);
                                }
                                None => {
                                    self.error("لا يوجد حرف بهذا الرقم", reporter);
                                    return Err(());
                                }
                            }
                        } else {
                            match string.chars().nth_back((idx + 1).abs() as usize) {
                                Some(value) => {
                                    self.push(Value::String(format!("{value}")));
                                    return Ok(1);
                                }
                                None => {
                                    self.error("لا يوجد عنصر بهذا الرقم", reporter);
                                    return Err(());
                                }
                            }
                        }
                    }
                    _ => {
                        self.error(
                            "يمكن استخدام هذا المعامل على القوائم والكائنات فقط",
                            reporter,
                        );
                        return Err(());
                    }
                }
            }
            Instruction::Set => {
                let key = self.pop();
                let popped = self.pop();

                match &popped {
                    Value::Object(items) => {
                        if !key.is_string() {
                            self.error("يجب أن يكون اسم الخاصية نصاً", reporter);
                            return Err(());
                        }

                        items.borrow_mut().insert(key.to_string(), self.last());
                    }
                    Value::List(items) => {
                        let idx: isize = match key.try_into() {
                            Ok(idx) => idx,
                            Err(_) => {
                                self.error("يجب أن يكون رقم العنصر عدداً صحيحاً", reporter);
                                return Err(());
                            }
                        };

                        if idx >= 0 {
                            match items.borrow_mut().get_mut(idx as usize) {
                                Some(value) => {
                                    *value = self.last();
                                    return Ok(1);
                                }
                                None => {
                                    self.error("لا يوجد عنصر بهذا الرقم", reporter);
                                    return Err(());
                                }
                            };
                        } else {
                            match items
                                .borrow_mut()
                                .iter_mut()
                                .nth_back((idx + 1).abs() as usize)
                            {
                                Some(value) => {
                                    *value = self.last();
                                    return Ok(1);
                                }
                                None => {
                                    self.error("لا يوجد عنصر بهذا الرقم", reporter);
                                    return Err(());
                                }
                            }
                        }
                    }
                    _ => {
                        self.error(
                            "يمكن استخدام هذا المعامل على القوائم والكائنات فقط",
                            reporter,
                        );
                        return Err(());
                    }
                }
            }
            Instruction::Closure => {
                let up_values_count = self.read_byte_oper() as usize;
                let function = self.pop().as_function();
                let up_values = {
                    let mut temp = Vec::with_capacity(up_values_count);
                    for idx in 0..up_values_count {
                        let offset = self.get_ip() + 2 + idx * 2;
                        temp.push((
                            self.get_byte(offset).unwrap() != 0,
                            self.get_byte(offset + 1).unwrap() as usize,
                        ))
                    }
                    temp
                };
                let closure = Closure::new(
                    function,
                    up_values
                        .into_iter()
                        .map(|(is_local, r_idx)| {
                            if is_local {
                                let idx = self.get_slots_start() + r_idx;

                                if let Some(up_value) = self
                                    .open_up_values
                                    .iter()
                                    .find(|up_value| up_value.borrow().as_open() == idx)
                                {
                                    Rc::clone(up_value)
                                } else {
                                    let up_value = Rc::new(RefCell::new(UpValue::new(idx)));
                                    self.open_up_values.push(Rc::clone(&up_value));
                                    up_value
                                }
                            } else {
                                self.frames
                                    .get(self.frames.len() - 2)
                                    .unwrap()
                                    .get_up_value(r_idx)
                            }
                        })
                        .collect::<Vec<Rc<RefCell<UpValue>>>>(),
                );

                self.push(Value::Closure(Rc::new(closure)));
                return Ok(2 + up_values_count * 2);
            }
            Instruction::Call => {
                let argc = self.read_byte_oper() as usize;

                self.last_frame_mut().ip += 2;

                match self.call(argc) {
                    Ok(()) => return Ok(0),
                    Err(err) => {
                        self.error(&err, reporter);
                        return Err(());
                    }
                }
            }
            Instruction::Return => {
                let returned = self.pop();
                let frame = self.frames.pop().unwrap();
                self.close_up_values(frame.slots_start);
                self.stack.truncate(frame.slots_start);

                if cfg!(feature = "debug-execution") {
                    let mut buffer = String::new();
                    buffer += format!("[DEBUG] returned from {:?}\n", frame).as_str();
                    buffer += format!("|       to {:?}", self.last_frame()).as_str();
                    println!("{}", buffer);
                }

                self.push(returned);
                return Ok(0);
            }
            Instruction::GetUpValue => {
                let idx = self.read_byte_oper() as usize;
                let up_value = self.last_frame().get_up_value(idx);

                self.push(match &*up_value.borrow() {
                    UpValue::Open(idx) => self.get(*idx),
                    UpValue::Closed(up_value) => up_value.clone(),
                });

                return Ok(2);
            }
            Instruction::SetUpValue => {
                let idx = self.read_byte_oper() as usize;
                let up_value = self.last_frame().get_up_value(idx);

                if up_value.borrow().is_open() {
                    *self.stack.get_mut(up_value.borrow().as_open()).unwrap() = self.last();
                } else {
                    *up_value.borrow_mut() = UpValue::Closed(self.last());
                }

                return Ok(2);
            }
            Instruction::CloseUpValue => {
                let idx = self.stack.len() - 1;
                self.close_up_values(idx);
                self.pop();
            }
        };
        Ok(1)
    }

    pub fn run(&mut self, reporter: &mut dyn Reporter) -> Result<(), ()> {
        if cfg!(feature = "debug-execution") {
            println!("---");
            println!("[DEBUG] started executing");
            println!("---");
        }

        while let Some(byte) = self.get_byte(self.get_ip()) {
            if cfg!(feature = "debug-execution") {
                print!(
                    "{}",
                    self.get_cur_chunk()
                        .disassemble_instr_at(self.get_ip(), false)
                        .0
                );
            }

            let instr = Instruction::try_from(byte).unwrap();
            let size = self.execute_instr(instr, reporter)?;
            self.last_frame_mut().ip += size;
        }

        Ok(())
    }
}
