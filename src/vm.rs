use super::{
    chunk::Instruction::{self, *},
    natives::NATIVES,
    path::resolve_path,
    reporter::{Phase, Report, Reporter},
    token::Token,
    utils::combine,
    value::{Arity, Closure, Function, Native, Object, UpValue, Value},
};
use std::{
    cell::RefCell, collections::HashMap, fmt, fs::File, path::PathBuf, rc::Rc, time::SystemTime,
};

pub struct Vm {
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
    open_up_values: Vec<Rc<RefCell<UpValue>>>,
    created_at: SystemTime,
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            stack: Vec::new(),
            globals: HashMap::new(),
            open_up_values: Vec::new(),
            created_at: SystemTime::now(),
        };

        NATIVES.iter().for_each(|(name, native)| {
            vm.globals
                .insert(name.to_string(), Value::new_native(native.clone()));
        });

        vm
    }

    fn get_up_value(&self, idx: usize) -> Option<Rc<RefCell<UpValue>>> {
        self.open_up_values
            .iter()
            .find(|up_value| up_value.borrow().as_open() == idx)
            .cloned()
    }

    fn append_up_value(&mut self, idx: usize) -> Rc<RefCell<UpValue>> {
        let up_value = Rc::new(RefCell::new(UpValue::new(idx)));
        self.open_up_values.push(Rc::clone(&up_value));
        up_value
    }

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

    pub fn check_arity(arity: Arity, argc: usize) -> Result<(), Value> {
        match arity {
            Arity::Fixed(arity) => {
                if argc != arity as usize {
                    Err(Value::new_string(format!(
                        "توقعت عدد {arity} من المدخلات ولكن حصلت على {argc} بدلاً من ذللك"
                    )))
                } else {
                    Ok(())
                }
            }
            Arity::Variadic(arity) => {
                if argc < arity as usize {
                    Err(Value::new_string(format!(
                        "توقعت على الأقل عدد {arity} من المدخلات ولكن حصلت على {argc} بدلاً من ذلك"
                    )))
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn run(&mut self, function: Function, reporter: &mut dyn Reporter) -> Result<(), ()> {
        if cfg!(feature = "debug-execution") {
            println!("---");
            println!("[DEBUG] started executing");
            println!("---");
        }

        let closure = Rc::new(Closure::new(Rc::new(function), Vec::new()));
        self.stack
            .push(Value::Object(Object::Closure(Rc::clone(&closure))));
        let res = Frame::new_closure(self, closure, 0, None).run(0); //TODO backtracing
        self.stack.pop();
        res.map_err(|(value, backtrace)| {
            reporter.error(Report::new(
                Phase::Runtime,
                value.to_string(),
                Rc::clone(&backtrace.frames[0].token),
            ));
            eprint!("{backtrace}");
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct BacktraceFrame {
    token: Rc<Token>,
    name: Option<String>,
}

impl BacktraceFrame {
    fn new(frame: &Frame) -> Self {
        if frame.is_native() {
            unreachable!();
        }
        Self {
            token: frame.cur_token(),
            name: frame.get_closure().get_name().clone(),
        }
    }
}

#[derive(Clone)]
struct Backtrace {
    frames: Vec<BacktraceFrame>,
}

impl Backtrace {
    fn new() -> Self {
        Self { frames: Vec::new() }
    }

    fn push(&mut self, frame: BacktraceFrame) {
        self.frames.push(frame);
    }
}

impl fmt::Display for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = String::new();
        let mut iter = self.frames.iter();
        while let Some(frame) = iter.next() {
            let (line, col) = frame.token.get_pos();
            let name = frame.name.clone().unwrap_or("غير معروفة".to_string());
            buffer += format!("في {name} {line}:{col}\n").as_str();
        }
        write!(f, "{buffer}")
    }
}

#[derive(Clone, Debug)]
pub struct Handler {
    slots: usize,
    ip: usize,
}

impl Handler {
    fn new(slots: usize, ip: usize) -> Self {
        Self { slots, ip }
    }
}

pub enum Frame<'a, 'b> {
    Closure {
        state: &'a mut Vm,
        closure: Rc<Closure>,
        ip: usize,
        slots: usize,
        enclosing_up_values: Option<&'b Vec<Rc<RefCell<UpValue>>>>,
        handlers: Vec<Handler>,
    },
    Native {
        state: &'a mut Vm,
        native: Native,
        slots: usize,
        path: Option<PathBuf>,
    },
}

impl<'a, 'b> Frame<'a, 'b> {
    fn new_closure(
        state: &'a mut Vm,
        closure: Rc<Closure>,
        slots: usize,
        enclosing_up_values: Option<&'b Vec<Rc<RefCell<UpValue>>>>,
    ) -> Self {
        Self::Closure {
            state,
            closure,
            ip: 0,
            slots,
            enclosing_up_values,
            handlers: Vec::new(),
        }
    }

    fn new_native(state: &'a mut Vm, native: Native, slots: usize, path: Option<PathBuf>) -> Self {
        Self::Native {
            state,
            native,
            slots,
            path,
        }
    }

    fn is_native(&self) -> bool {
        match self {
            Self::Native { .. } => true,
            _ => false,
        }
    }

    fn get_closure(&self) -> &Rc<Closure> {
        match self {
            Self::Closure { closure, .. } => closure,
            Self::Native { .. } => unreachable!(),
        }
    }

    fn get_ip(&self) -> usize {
        match self {
            Self::Closure { ip, .. } => *ip,
            Self::Native { .. } => unreachable!(),
        }
    }

    fn set_ip(&mut self, next: usize) {
        match self {
            Self::Closure { ip, .. } => *ip = next,
            Self::Native { .. } => unreachable!(),
        }
    }

    fn get_enclosing_up_values(&self) -> &Option<&Vec<Rc<RefCell<UpValue>>>> {
        match self {
            Self::Closure {
                enclosing_up_values,
                ..
            } => enclosing_up_values,
            Self::Native { .. } => unreachable!(),
        }
    }

    fn get_handlers_mut(&mut self) -> &mut Vec<Handler> {
        match self {
            Self::Closure { handlers, .. } => handlers,
            Self::Native { .. } => unreachable!(),
        }
    }

    fn get_state_mut(&mut self) -> &mut Vm {
        match self {
            Self::Closure { state, .. } => state,
            Self::Native { state, .. } => state,
        }
    }

    fn get_state(&self) -> &Vm {
        match self {
            Self::Closure { state, .. } => state,
            Self::Native { state, .. } => state,
        }
    }

    fn get_native(&self) -> Native {
        match self {
            Self::Closure { .. } => unreachable!(),
            Self::Native { native, .. } => native.clone(),
        }
    }

    fn get_slots(&self) -> usize {
        match self {
            Self::Closure { slots, .. } => *slots,
            Self::Native { slots, .. } => *slots,
        }
    }

    fn get_path(&self) -> &Option<PathBuf> {
        match self {
            Self::Closure { closure, .. } => closure.get_path(),
            Self::Native { path, .. } => path,
        }
    }

    fn read_byte(&self) -> usize {
        self.get_closure()
            .get_chunk()
            .get_byte(self.get_ip() + 1)
            .unwrap() as usize
    }

    fn read_up_value(&self, offset: usize) -> (bool, usize) {
        (
            self.get_closure().get_chunk().get_byte(offset).unwrap() != 0,
            self.get_closure().get_chunk().get_byte(offset + 1).unwrap() as usize,
        )
    }

    fn read_2bytes(&self) -> usize {
        combine(
            self.get_closure()
                .get_chunk()
                .get_byte(self.get_ip() + 1)
                .unwrap(),
            self.get_closure()
                .get_chunk()
                .get_byte(self.get_ip() + 2)
                .unwrap(),
        ) as usize
    }

    fn cur_byte(&self) -> Option<u8> {
        self.get_closure().get_chunk().get_byte(self.get_ip())
    }

    fn cur_instr(&self) -> Option<Instruction> {
        Some(self.cur_byte()?.into())
    }

    fn cur_token(&self) -> Rc<Token> {
        self.get_closure().get_chunk().get_token(self.get_ip())
    }

    fn pop(&mut self) -> Value {
        self.get_state_mut().stack.pop().unwrap()
    }

    fn push(&mut self, value: Value) {
        self.get_state_mut().stack.push(value);
    }

    fn last(&self) -> &Value {
        self.get_state().stack.last().unwrap()
    }

    fn get(&self, idx: usize) -> &Value {
        &self.get_state().stack[idx]
    }

    fn get_mut(&mut self, idx: usize) -> &mut Value {
        &mut self.get_state_mut().stack[idx]
    }

    fn truncate(&mut self, len: usize) {
        self.get_state_mut().close_up_values(len);
        self.get_state_mut().stack.truncate(len);
    }

    //>> Native functions utilities
    pub fn nth(&self, idx: usize) -> &Value {
        self.get(self.get_slots() + idx)
    }

    pub fn nth_f64(&self, idx: usize) -> Result<f64, Value> {
        if let Value::Number(n) = self.nth(idx) {
            Ok(*n)
        } else {
            Err(format!("يجب أن يكون المدخل {idx} عدداً").into())
        }
    }

    pub fn nth_i32(&self, idx: usize) -> Result<i32, Value> {
        if let Value::Number(n) = self.nth(idx) {
            if n.fract() == 0.0 {
                Ok(*n as i32)
            } else {
                Err(format!("يجب أن يكون المدخل {idx} عدداً صحيحاً").into())
            }
        } else {
            Err(format!("يجب أن يكون المدخل {idx} عدداً صحيحاً").into())
        }
    }

    pub fn nth_u32(&self, idx: usize) -> Result<u32, Value> {
        if let Value::Number(n) = self.nth(idx) {
            if n.fract() == 0.0 && *n > 0.0 {
                Ok(*n as u32)
            } else {
                Err(format!("يجب أن يكون المدخل {idx} عدداً صحيحاً موجباً").into())
            }
        } else {
            Err(format!("يجب أن يكون المدخل {idx} عدداً صحيحاً موجباً").into())
        }
    }

    pub fn nth_string(&self, idx: usize) -> Result<&str, Value> {
        if let Value::Object(Object::String(string)) = self.nth(idx) {
            Ok(string)
        } else {
            Err(format!("يجب أن يكون المدخل {idx} نص").into())
        }
    }

    pub fn nth_char(&self, idx: usize) -> Result<char, Value> {
        if let Value::Object(Object::String(string)) = self.nth(idx) {
            if string.chars().count() == 1 {
                Ok(string.chars().nth(0).unwrap())
            } else {
                Err(format!("يجب أن يكون المدخل {idx} نص ذي حرف واحد").into())
            }
        } else {
            Err(format!("يجب أن يكون المدخل {idx} نص ذي حرف واحد").into())
        }
    }

    pub fn nth_object(&self, idx: usize) -> Result<&Rc<RefCell<HashMap<String, Value>>>, Value> {
        if let Value::Object(Object::Object(items)) = self.nth(idx) {
            Ok(items)
        } else {
            Err(format!("يجب أن يكون المدخل {idx} مجموعة").into())
        }
    }

    pub fn nth_list(&self, idx: usize) -> Result<&Rc<RefCell<Vec<Value>>>, Value> {
        if let Value::Object(Object::List(items)) = self.nth(idx) {
            Ok(items)
        } else {
            Err(format!("يجب أن يكون المدخل {idx} قائمة").into())
        }
    }

    pub fn nth_file(&self, idx: usize) -> Result<&Rc<RefCell<File>>, Value> {
        if let Value::Object(Object::File(file)) = self.nth(idx) {
            Ok(file)
        } else {
            Err(format!("يجب أن يكون المدخل {idx} ملف").into())
        }
    }

    pub fn nth_path(&self, idx: usize) -> Result<PathBuf, Value> {
        if let Value::Object(Object::String(string)) = self.nth(idx) {
            resolve_path(self.get_path().clone(), &string, |_| Ok(()))
                .map_err(|string| string.into())
        } else {
            Err(format!("يجب أن يكون المدخل {idx} مسار").into())
        }
    }

    pub fn get_creation_time(&self) -> &SystemTime {
        &self.get_state().created_at
    }
    //<<

    fn string_to_err(&self, string: String) -> (Value, Backtrace) {
        let mut backtrace = Backtrace::new();
        backtrace.push(BacktraceFrame::new(self));
        (string.to_string().into(), backtrace)
    }

    fn run(&mut self, argc: usize) -> Result<Option<Value>, (Value, Backtrace)> {
        if self.is_native() {
            let returned =
                self.get_native()(self, argc).map_err(|value| (value, Backtrace::new()))?;
            return Ok(Some(returned));
        }

        fn get_absolute_idx(idx: i32, len: usize) -> Result<usize, ()> {
            if idx >= 0 {
                if idx >= len as i32 {
                    return Err(());
                }
                Ok(idx as usize)
            } else {
                if -idx > len as i32 {
                    return Err(());
                }
                Ok((len as i32 + idx) as usize)
            }
        }

        while let Some(instr) = self.cur_instr() {
            if cfg!(feature = "debug-execution") {
                print!(
                    "{}",
                    self.get_closure()
                        .get_chunk()
                        .disassemble_instr_at(self.get_ip(), false)
                        .0
                );
            }

            let mut progress = 1i32;

            match instr {
                Pop => {
                    self.pop();
                }
                Constant8 => {
                    let idx = self.read_byte();
                    self.push(self.get_closure().get_chunk().get_constant(idx));
                    progress = 2;
                }
                Constant16 => {
                    let idx = self.read_2bytes();
                    self.push(self.get_closure().get_chunk().get_constant(idx));
                    progress = 3;
                }
                Negate => {
                    let popped = self.pop();
                    if !popped.is_number() {
                        return Err(self.string_to_err("يجب أن يكون المعامل رقماً".to_string()));
                    }
                    self.push(-popped);
                }
                Add => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a + b);
                }
                Subtract => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_subtractable(&a, &b) {
                        return Err(
                            self.string_to_err("لا يقبل المعاملان الطرح من بعضهما".to_string())
                        );
                    }
                    self.push(a - b);
                }
                Multiply => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_multipliable(&a, &b) {
                        return Err(
                            self.string_to_err("لا يقبل المعاملان الضرب في بعضهما".to_string())
                        );
                    }
                    self.push(a * b);
                }
                Divide => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_dividable(&a, &b) {
                        return Err(
                            self.string_to_err("لا يقبل المعاملان القسمة على بعضهما".to_string())
                        );
                    }
                    self.push(a / b);
                }
                Remainder => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_remainderable(&a, &b) {
                        return Err(
                            self.string_to_err("لا يقبل المعاملان القسمة على بعضهما".to_string())
                        );
                    }
                    self.push(a % b);
                }
                Not => {
                    let popped = self.pop();
                    self.push(!popped);
                }
                Equal => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(Value::Bool(a == b));
                }
                Greater => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_numbers(&a, &b) {
                        return Err(self.string_to_err("يجب أن يكون المعاملان أرقاماً".to_string()));
                    }
                    self.push(Value::Bool(a > b));
                }
                GreaterEqual => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_numbers(&a, &b) {
                        return Err(self.string_to_err("يجب أن يكون المعاملان أرقاماً".to_string()));
                    }
                    self.push(Value::Bool(a >= b));
                }
                Less => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_numbers(&a, &b) {
                        return Err(self.string_to_err("يجب أن يكون المعاملان أرقاماً".to_string()));
                    }
                    self.push(Value::Bool(a < b));
                }
                LessEqual => {
                    let b = self.pop();
                    let a = self.pop();
                    if !Value::are_numbers(&a, &b) {
                        return Err(self.string_to_err("يجب أن يكون المعاملان أرقاماً".to_string()));
                    }
                    self.push(Value::Bool(a <= b));
                }
                Jump => {
                    progress = self.read_2bytes() as i32;
                }
                JumpIfFalse => {
                    if self.last().is_truthy() {
                        progress = 3;
                    } else {
                        progress = self.read_2bytes() as i32;
                    }
                }
                JumpIfTrue => {
                    if !self.last().is_truthy() {
                        progress = 3;
                    } else {
                        progress = self.read_2bytes() as i32;
                    }
                }
                Loop => {
                    progress = -(self.read_2bytes() as i32);
                }
                DefineGlobal => {
                    let name = self.pop().to_string();
                    let value = self.pop();
                    if self.get_state().globals.contains_key(&name) {
                        return Err(self.string_to_err("يوجد متغير بهذا الاسم".to_string()));
                    }
                    self.get_state_mut().globals.insert(name.clone(), value);
                }
                SetGlobal => {
                    let name = self.pop().to_string();
                    let value = self.last().clone();
                    if !self.get_state().globals.contains_key(&name) {
                        return Err(self.string_to_err("لا يوجد متغير بهذا الاسم".to_string()));
                    }
                    self.get_state_mut().globals.insert(name, value);
                }
                GetGlobal => {
                    let name = self.pop().to_string();
                    if !self.get_state().globals.contains_key(&name) {
                        return Err(self.string_to_err("لا يوجد متغير بهذا الاسم".to_string()));
                    }
                    self.push(self.get_state().globals.get(&name).unwrap().clone());
                }
                GetLocal => {
                    let idx = self.get_slots() + self.read_byte();
                    self.push(self.get(idx).clone());
                    progress = 2;
                }
                SetLocal => {
                    let idx = self.get_slots() + self.read_byte();
                    *self.get_mut(idx) = self.last().clone();
                    progress = 2;
                }
                BuildList => {
                    let size = self.read_byte();
                    let mut items = Vec::new();
                    let len = self.get_state().stack.len();
                    for item in self.get_state_mut().stack.drain(len - size..) {
                        items.push(item);
                    }
                    self.push(Value::new_list(items));
                    progress = 2;
                }
                BuildObject => {
                    let size = self.read_byte();
                    let mut items = HashMap::new();
                    for _ in 0..size {
                        let value = self.pop();
                        let name = self.pop().to_string();
                        items.insert(name, value);
                    }
                    self.push(Value::new_object(items));
                    progress = 2;
                }
                Get => {
                    let key = self.pop();
                    let obj = self.pop();
                    match obj {
                        Value::Object(Object::Object(items)) => {
                            if !key.is_string() {
                                return Err(
                                    self.string_to_err("يجب أن يكون اسم الخاصية نصاً".to_string())
                                );
                            }
                            if let Some(value) = items.borrow().get(&key.to_string()) {
                                self.push(value.clone());
                            } else {
                                return Err(
                                    self.string_to_err("لا يوجد قيمة بهذا الاسم".to_string())
                                );
                            }
                        }
                        Value::Object(Object::List(items)) => {
                            if !key.is_int() {
                                return Err(self.string_to_err(
                                    "يجب أن يكون رقم العنصر عدداً صحيحاً".to_string(),
                                ));
                            }
                            let idx = get_absolute_idx(key.as_int(), items.borrow().len())
                                .map_err(|_| {
                                    self.string_to_err("لا يوجد عنصر بهذا الرقم".to_string())
                                })?;
                            self.push(items.borrow()[idx].clone());
                        }
                        Value::Object(Object::String(string)) => {
                            if !key.is_int() {
                                return Err(self.string_to_err(
                                    "يجب أن يكون رقم العنصر عدداً صحيحاً".to_string(),
                                ));
                            }
                            let idx = get_absolute_idx(key.as_int(), string.chars().count())
                                .map_err(|_| {
                                    self.string_to_err("لا يوجد عنصر بهذا الرقم".to_string())
                                })?;
                            self.push(Value::new_string(
                                string.chars().nth(idx).unwrap().to_string(),
                            ));
                        }
                        _ => {
                            return Err(self.string_to_err(
                                "يجب أن يكون المتغير نص أو قائمة أو كائن".to_string(),
                            ))
                        }
                    }
                }
                Set => {
                    let key = self.pop();
                    let obj = self.pop();
                    match obj {
                        Value::Object(Object::Object(items)) => {
                            if !key.is_string() {
                                return Err(
                                    self.string_to_err("يجب أن يكون اسم الخاصية نصاً".to_string())
                                );
                            }
                            items
                                .borrow_mut()
                                .insert(key.as_string(), self.last().clone());
                        }
                        Value::Object(Object::List(items)) => {
                            if !key.is_int() {
                                return Err(self.string_to_err(
                                    "يجب أن يكون رقم العنصر عدداً صحيحاً".to_string(),
                                ));
                            }

                            let idx = get_absolute_idx(key.as_int(), items.borrow().len())
                                .map_err(|_| {
                                    self.string_to_err("لا يوجد عنصر بهذا الرقم".to_string())
                                })?;

                            items.borrow_mut()[idx] = self.last().clone();
                        }
                        _ => {
                            return Err(
                                self.string_to_err("يجب أن يكون المتغير قائمة أو كائن".to_string())
                            )
                        }
                    }
                }
                Closure => {
                    //TODO test
                    let count = self.read_byte() as usize;
                    let function = self.pop().as_function();
                    let up_values = {
                        let mut data = Vec::with_capacity(count);
                        for idx in 0..count {
                            let offset = self.get_ip() + 2 + idx * 2;
                            data.push(self.read_up_value(offset))
                        }

                        let mut res = Vec::with_capacity(count);

                        for (is_local, mut idx) in data {
                            if is_local {
                                idx = self.get_slots() + idx;
                                if let Some(up_value) = self.get_state().get_up_value(idx) {
                                    res.push(up_value);
                                } else {
                                    res.push(self.get_state_mut().append_up_value(idx))
                                }
                            } else {
                                res.push(self.get_enclosing_up_values().unwrap()[idx].clone());
                            }
                        }

                        res
                    };
                    self.push(Value::new_closure(function, up_values));
                    progress = 2 + count as i32 * 2;
                }
                Call => {
                    let argc = self.read_byte();
                    let idx = self.get_state().stack.len() - argc - 1;
                    let enclosing_closure = self.get_closure().clone();
                    let mut frame = match self.get_state().stack[idx].clone() {
                        Value::Object(Object::Closure(closure)) => {
                            Vm::check_arity(closure.get_arity(), argc)
                                .map_err(|value| self.string_to_err(value.to_string()))?;
                            Frame::new_closure(
                                self.get_state_mut(),
                                closure,
                                idx,
                                Some(enclosing_closure.get_up_values()),
                            )
                        }
                        Value::Object(Object::Native(native)) => {
                            let path = self.get_path().clone();

                            Frame::new_native(self.get_state_mut(), native, idx, path)
                        }
                        _ => todo!(),
                    };
                    progress = 2;
                    match frame.run(argc) {
                        Ok(returned) => {
                            self.truncate(idx);
                            self.push(returned.unwrap());
                        }
                        Err((value, mut backtrace)) => match self.get_handlers_mut().pop() {
                            Some(Handler { slots, ip }) => {
                                self.truncate(slots);
                                self.push(value);
                                progress = (ip - self.get_ip()) as i32;
                            }
                            None => {
                                backtrace.push(BacktraceFrame::new(self));
                                return Err((value, backtrace));
                            }
                        },
                    };
                }
                GetUpValue => {
                    let idx = self.read_byte();
                    let up_value = self.get_closure().get_up_value(idx);
                    self.push(match &*up_value.borrow() {
                        UpValue::Open(idx) => self.get(*idx).clone(),
                        UpValue::Closed(up_value) => up_value.clone(),
                    });
                    progress = 2;
                }
                SetUpValue => {
                    let idx = self.read_byte();
                    let up_value = self.get_closure().get_up_value(idx);
                    if up_value.borrow().is_open() {
                        *self.get_mut(up_value.borrow().as_open()) = self.last().clone();
                    } else {
                        *up_value.borrow_mut() = UpValue::Closed(self.last().clone());
                    }
                    progress = 2;
                }
                CloseUpValue => {
                    let idx = self.get_state().stack.len() - 1;
                    self.get_state_mut().close_up_values(idx);
                    self.pop();
                }
                Return => return Ok(Some(self.pop())),
                AppendHandler => {
                    let handler = Handler::new(
                        self.get_state().stack.len(),
                        self.get_ip() + self.read_byte(),
                    );
                    self.get_handlers_mut().push(handler);
                    progress = 3;
                }
                PopHandler => {
                    self.get_handlers_mut().pop();
                }
                Throw => match self.get_handlers_mut().pop() {
                    Some(Handler { slots, ip }) => {
                        let throwed = self.pop();
                        self.truncate(slots);
                        self.push(throwed);
                        progress = (ip - self.get_ip()) as i32;
                    }
                    None => {
                        let mut backtrace = Backtrace::new();
                        backtrace.push(BacktraceFrame::new(self));
                        return Err((self.pop(), backtrace));
                    }
                },
                Unknown => unreachable!(),
            }
            self.set_ip((self.get_ip() as i32 + progress) as usize);
            if cfg!(feature = "debug-execution") {
                println!("{:#?}", self.get_state().stack);
            }
        }
        Ok(None)
    }
}
