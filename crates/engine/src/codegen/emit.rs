use super::{Codegen, parallel};
use crate::parser::Node;

pub fn emit_nodes(g: &mut Codegen, s: &str, nodes: &[Node]) {
    for n in nodes {
        emit_node(g, s, n);
    }
}

fn emit_node(g: &mut Codegen, s: &str, n: &Node) {
    match n {
        Node::IncPtr => g.line(&format!("call void @bf_inc_ptr(%State* {s}, i64 1)")),
        Node::DecPtr => g.line(&format!("call void @bf_inc_ptr(%State* {s}, i64 -1)")),
        Node::IncCell => g.line(&format!("call void @bf_add_cell(%State* {s}, i32 1)")),
        Node::DecCell => g.line(&format!("call void @bf_add_cell(%State* {s}, i32 -1)")),
        Node::Output => g.line(&format!("call void @bf_output(%State* {s})")),
        Node::Input => g.line(&format!("call void @bf_input(%State* {s})")),
        Node::LockAcquire => g.line(&format!("call void @bf_lock_acquire(%State* {s})")),
        Node::LockRelease => g.line(&format!("call void @bf_lock_release(%State* {s})")),
        Node::Sleep(t) => g.line(&format!("call void @bf_sleep(i32 {t})")),
        Node::Loop(body) => emit_loop(g, s, body),
        Node::Parallel(bs) => parallel::emit_parallel(g, s, bs),
        _ => {} // TODO: Handle Node::Wait and Node::Notify
    }
}

fn emit_loop(g: &mut Codegen, s: &str, body: &[Node]) {
    let id = g.uniq;
    g.uniq += 1;
    let l_cond = format!("loop.cond.{id}");
    let l_body = format!("loop.body.{id}");
    let l_end = format!("loop.end.{id}");

    g.line(&format!("br label %{l_cond}"));
    g.label(&l_cond);
    g.line(&format!("%p{id} = call i8* @bf_gep_cell_ptr(%State* {s})"));
    if g.sanitize {
        g.line(&format!("call void @tsan_read(%State* {s})"));
    }
    g.line(&format!("%v{id} = load i8, i8* %p{id}"));
    g.line(&format!("%nz{id} = icmp ne i8 %v{id}, 0"));
    g.line(&format!("br i1 %nz{id}, label %{l_body}, label %{l_end}"));

    g.label(&l_body);
    emit_nodes(g, s, body);
    g.line(&format!("br label %{l_cond}"));

    g.label(&l_end);
}
