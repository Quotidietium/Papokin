use papokin_protocol::{
    codec::var_int::VarInt,
    java::client::play::{CCommands, ProtoNode, ProtoNodeType},
};
use std::sync::Arc;

use crate::command::node::{
    attached::AttachedNode, dispatcher::CommandDispatcher, tree::ROOT_NODE_ID,
};
use crate::entity::player::Player;
use crate::server::Server;
use papokin_protocol::java::client::play::SuggestionProviders;

#[allow(clippy::too_many_lines)]
pub fn send_c_commands_packet(
    player: &Arc<Player>,
    server: &Arc<Server>,
    dispatcher: &CommandDispatcher,
) {
    let mut proto_nodes: Vec<ProtoNode> = Vec::with_capacity(dispatcher.tree.len());
    let source = &super::CommandSender::Player(player.clone()).into_source(server);
    for node in &dispatcher.tree {
        let children: Box<[VarInt]> = match node {
            AttachedNode::Root(_) => {
                // 从根节点的子列表中移除已禁用的命令，以便它们
                // 从客户端的命令图（及 Tab 补全）中消失
                // 完全。
                node.children_ref()
                    .values()
                    .copied()
                    .filter(|id| {
                        let (disabled, requirement, name) = match &dispatcher.tree[*id] {
                            AttachedNode::Literal(child) => (
                                dispatcher.is_disabled(&child.meta.literal_lowercase),
                                child.owned.requirements.evaluate(source),
                                child.meta.literal.as_ref(),
                            ),
                            AttachedNode::Command(child) => (
                                dispatcher.is_disabled(&child.meta.literal_lowercase),
                                child.owned.requirements.evaluate(source),
                                child.meta.literal.as_ref(),
                            ),
                            _ => (false, true, ""),
                        };
                        !disabled
                            && requirement
                            && (!name.starts_with("//")
                                || dispatcher.tree.get(&name[1..]).is_none())
                    })
                    .map(|id| VarInt((id.0.get() - 1) as i32))
                    .collect()
            }
            _ => node
                .children_ref()
                .values()
                .copied()
                .map(|id| VarInt((id.0.get() - 1) as i32))
                .collect(),
        };

        let redirect_target = node
            .redirect()
            .and_then(|redirection| dispatcher.tree.resolve(redirection))
            .map(|id| (id.0.get() - 1) as i32);

        let satisfies_requirements = node.requirements().evaluate(source);

        match node {
            AttachedNode::Root(_) => {
                proto_nodes.push(ProtoNode {
                    children,
                    node_type: ProtoNodeType::Root,
                });
            }
            AttachedNode::Literal(literal_attached_node) => {
                let name = if literal_attached_node.meta.literal.starts_with("//") {
                    &literal_attached_node.meta.literal[1..]
                } else {
                    &literal_attached_node.meta.literal
                };
                let node = ProtoNode {
                    children,
                    node_type: ProtoNodeType::Literal {
                        name,
                        is_executable: literal_attached_node.owned.command.is_some(),
                        redirect_target,
                        restricted: !satisfies_requirements,
                    },
                };
                proto_nodes.push(node);
            }
            AttachedNode::Command(command_attached_node) => {
                let name = if command_attached_node.meta.literal.starts_with("//") {
                    &command_attached_node.meta.literal[1..]
                } else {
                    &command_attached_node.meta.literal
                };
                let node = ProtoNode {
                    children,
                    node_type: ProtoNodeType::Literal {
                        name,
                        is_executable: command_attached_node.owned.command.is_some(),
                        redirect_target,
                        restricted: !satisfies_requirements,
                    },
                };
                proto_nodes.push(node);
            }
            AttachedNode::Argument(argument_attached_node) => {
                let arg_type = &argument_attached_node.meta.argument_type;

                let node = ProtoNode {
                    children,
                    node_type: ProtoNodeType::Argument {
                        name: &argument_attached_node.meta.name,
                        is_executable: argument_attached_node.owned.command.is_some(),
                        parser: arg_type.client_side_parser(),
                        override_suggestion_type: if argument_attached_node
                            .meta
                            .suggestion_provider
                            .is_some()
                        {
                            Some(SuggestionProviders::AskServer)
                        } else {
                            arg_type.override_suggestion_providers()
                        },
                        redirect_target,
                        restricted: !satisfies_requirements,
                    },
                };
                proto_nodes.push(node);
            }
        }
    }

    let root_node_index = ROOT_NODE_ID.0.get() - 1;
    let packet = CCommands::new(proto_nodes.into(), VarInt(root_node_index as i32));
    player.try_send_client_packet(&packet);
}
