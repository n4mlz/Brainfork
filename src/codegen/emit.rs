use super::{Codegen, parallel};
use crate::parser::Node;

pub fn emit_nodes(g: &mut Codegen, s: &str, nodes: &[Node]) {
    for n in nodes {
        emit_node(g, s, n);
    }
}

fn emit_node(g: &mut Codegen, s: &str, n: &Node) {
    match n {
        Node::IncPtr => g.wln(&format!("call void @bf_inc_ptr(%State* {s}, i64 1)")),
        Node::DecPtr => g.wln(&format!("call void @bf_inc_ptr(%State* {s}, i64 -1)")),
        Node::IncCell => g.wln(&format!("call void @bf_add_cell(%State* {s}, i32 1)")),
        Node::DecCell => g.wln(&format!("call void @bf_add_cell(%State* {s}, i32 -1)")),
        Node::Output => g.wln(&format!("call void @bf_output(%State* {s})")),
        Node::Input => g.wln(&format!("call void @bf_input(%State* {s})")),
        Node::LockAcquire => g.wln(&format!("call void @bf_lock_acquire(%State* {s})")),
        Node::LockRelease => g.wln(&format!("call void @bf_lock_release(%State* {s})")),
        Node::Wait(t) => g.wln(&format!("call void @bf_wait(i32 {t})")),
        Node::Loop(body) => emit_loop(g, s, body),
        Node::Parallel(bs) => parallel::emit_parallel(g, s, bs),
    }
}

fn emit_loop(g: &mut Codegen, s: &str, body: &[Node]) {
    let id = g.uniq;
    g.uniq += 1;
    let l_cond = format!("loop.cond.{id}");
    let l_body = format!("loop.body.{id}");
    let l_end = format!("loop.end.{id}");

    g.wln(&format!("br label %{l_cond}"));
    g.label(&l_cond);
    g.wln(&format!("%p{id} = call i8* @bf_gep_cell_ptr(%State* {s})"));
    g.wln(&format!("%v{id} = load i8, i8* %p{id}"));
    g.wln(&format!("%nz{id} = icmp ne i8 %v{id}, 0"));
    g.wln(&format!("br i1 %nz{id}, label %{l_body}, label %{l_end}"));

    g.label(&l_body);
    emit_nodes(g, s, body);
    g.wln(&format!("br label %{l_cond}"));

    g.label(&l_end);
}
