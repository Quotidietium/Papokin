use dashmap::DashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

/// 描述权限的默认行为。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDefault {
    /// 默认不授予该权限。
    Deny,
    /// 默认授予该权限。
    Allow,
    /// 默认向管理员（op）授予该权限。
    Op(PermissionLvl),
}

/// 在系统中定义一个权限节点。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Permission {
    /// 完整的节点名称（例如 "minecraft:command.gamemode"）。
    pub node: String,
    /// 描述此权限的作用。
    pub description: String,
    /// 此权限的默认值。
    pub default: PermissionDefault,
    /// 受此权限影响的子节点。
    pub children: HashMap<String, bool>,
}

impl Permission {
    /// 创建新的 `Permission` 实例。
    ///
    /// # Parameters
    /// - `node`：完整的权限节点字符串（例如 `"minecraft:command.gamemode"`）。
    /// - `description`：描述此权限用途的人类可读说明。
    /// - `default`：权限的默认行为（`PermissionDefault`）。
    ///
    /// # Returns
    /// 一个子权限集合为空的新 `Permission`。
    #[must_use]
    pub fn new(node: &str, description: &str, default: PermissionDefault) -> Self {
        Self {
            node: node.to_string(),
            description: description.to_string(),
            default,
            children: HashMap::new(),
        }
    }

    /// 向此权限添加一个子权限节点。
    ///
    /// # Arguments
    /// * `child` - 子节点名称。
    /// * `value` - 子节点是否默认被允许。
    ///
    /// # Returns
    /// 返回自身的可变引用以便链式调用。
    pub fn add_child(&mut self, child: &str, value: bool) -> &mut Self {
        self.children.insert(child.to_string(), value);
        self
    }
}

/// 服务器中所有已注册权限的仓库。
#[derive(Default)]
pub struct PermissionRegistry {
    /// 所有已注册的权限。
    permissions: DashMap<String, Permission>,
}

impl PermissionRegistry {
    /// 创建新的空 `PermissionRegistry`。
    ///
    /// # Returns
    /// 一个未注册任何权限的 `PermissionRegistry`。
    #[must_use]
    pub fn new() -> Self {
        Self {
            permissions: DashMap::new(),
        }
    }

    /// 在注册表中注册一个新权限。
    ///
    /// # Parameters
    /// - `permission`：要添加的 `Permission` 实例。
    ///
    /// # Returns
    /// - 若权限注册成功，则返回 `Ok(())`。
    /// - 若已存在相同节点的权限，则返回 `Err(String)`。
    pub fn register_permission(&self, permission: Permission) -> Result<(), String> {
        if self.permissions.contains_key(&permission.node) {
            return Err(format!(
                "Permission {} is already registered",
                permission.node
            ));
        }
        self.permissions.insert(permission.node.clone(), permission);
        Ok(())
    }

    /// 在注册表中注册一个新权限，并期望它已被注册。
    ///
    /// # Panics
    ///
    /// 如果权限无法注册（已存在相同节点的权限）则 panic。
    ///
    /// # Parameters
    /// - `permission`：要添加的 `Permission` 实例。
    #[allow(clippy::expect_used)]
    pub fn register_permission_or_panic(&self, permission: Permission) {
        self.register_permission(permission)
            .expect("权限应当已成功注册");
    }

    /// 按名称检索权限节点。
    ///
    /// # Parameters
    /// - `node`：要查找的完整权限节点字符串。
    ///
    /// # Returns
    /// 节点存在时返回 `Some(Ref<'_, String, Permission>)`，否则返回 `None`。
    #[must_use]
    pub fn get_permission(
        &self,
        node: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, Permission>> {
        self.permissions.get(node)
    }

    /// 覆盖某个已注册权限的默认行为。
    ///
    /// 用于应用来自服务器的逐命令权限覆盖
    /// 配置，位于内置权限注册之后。
    ///
    /// # Parameters
    /// - `node`：要更新的完整权限节点字符串。
    /// - `default`：要应用的新默认行为。
    ///
    /// # Returns
    /// - 若节点已存在且其默认值已被更新，则为 `true`。
    /// - 若未注册具有该节点的权限，则返回 `false`。
    #[must_use]
    pub fn set_default(&self, node: &str, default: PermissionDefault) -> bool {
        if let Some(mut permission) = self.permissions.get_mut(node) {
            permission.default = default;
            true
        } else {
            false
        }
    }

    /// 检查权限节点是否存在于注册表中。
    ///
    /// # Parameters
    /// - `node`：要检查的权限节点字符串。
    ///
    /// # Returns
    /// 如果节点存在则为 `true`，否则为 `false`。
    #[must_use]
    pub fn has_permission(&self, node: &str) -> bool {
        self.permissions.contains_key(node)
    }
}

/// 玩家权限的存储。
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct PermissionAttachment {
    /// 直接分配的权限。
    pub permissions: HashMap<String, bool>,
}

impl PermissionAttachment {
    /// 创建新的空 `PermissionAttachment`。
    ///
    /// # Returns
    /// 一个未设置任何权限的 `PermissionAttachment`。
    #[must_use]
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
        }
    }

    /// 为特定权限节点设置权限值。
    ///
    /// # Parameters
    /// - `node`：权限节点字符串。
    /// - `value`：权限是被授予（`true`）还是被拒绝（`false`）。
    pub fn set_permission(&mut self, node: &str, value: bool) {
        self.permissions.insert(node.to_string(), value);
    }

    /// 从此附件中移除一个权限。
    ///
    /// # Parameters
    /// - `node`：要移除的权限节点字符串。
    pub fn unset_permission(&mut self, node: &str) {
        self.permissions.remove(node);
    }

    /// 检查某项权限是否已被显式设置。
    ///
    /// # Parameters
    /// - `node`：要查询的权限节点字符串。
    ///
    /// # Returns
    /// 已授权返回 `Some(true)`，已拒绝返回 `Some(false)`，未设置则返回 `None`。
    #[must_use]
    pub fn has_permission_set(&self, node: &str) -> Option<bool> {
        self.permissions.get(node).copied()
    }

    ///返回对所有已设置权限的引用。
    ///
    /// # Returns
    /// 一个 `&HashMap<String, bool>`，包含所有权限节点及其值。
    #[must_use]
    pub const fn get_permissions(&self) -> &HashMap<String, bool> {
        &self.permissions
    }
}

/// 玩家与服务器权限的管理器。
#[derive(Default)]
pub struct PermissionManager {
    /// 权限的全局注册表。
    pub registry: PermissionRegistry,
    /// 以玩家 UUID 为键的玩家权限附件。
    pub attachments: DashMap<uuid::Uuid, DashMap<String, bool>>,
}

impl PermissionManager {
    /// 创建新的空 `PermissionManager`。
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: PermissionRegistry::new(),
            attachments: DashMap::new(),
        }
    }

    /// 使用现有的 `PermissionRegistry` 创建新的 `PermissionManager`。
    #[must_use]
    pub fn with_registry(registry: PermissionRegistry) -> Self {
        Self {
            registry,
            attachments: DashMap::new(),
        }
    }

    /// 在全局注册表中注册一个新权限节点。
    pub fn register_permission(&self, permission: Permission) -> Result<(), String> {
        self.registry.register_permission(permission)
    }

    /// 在全局注册表中注册一个新权限节点，若已注册则 panic。
    pub fn register_permission_or_panic(&self, permission: Permission) {
        self.registry.register_permission_or_panic(permission);
    }

    /// 按名称从注册表检索权限节点。
    #[must_use]
    pub fn get_permission(
        &self,
        node: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, Permission>> {
        self.registry.get_permission(node)
    }

    /// 覆盖注册表中已注册权限的默认行为。
    #[must_use]
    pub fn set_default(&self, node: &str, default: PermissionDefault) -> bool {
        self.registry.set_default(node, default)
    }

    /// 检查权限节点是否存在于注册表中。
    #[must_use]
    pub fn has_registered_permission(&self, node: &str) -> bool {
        self.registry.has_permission(node)
    }

    /// 设置玩家的显式权限。
    pub fn set_permission(&self, player_id: uuid::Uuid, node: impl Into<String>, value: bool) {
        self.attachments
            .entry(player_id)
            .or_default()
            .insert(node.into(), value);
    }

    /// 取消玩家的显式权限。
    pub fn unset_permission(&self, player_id: &uuid::Uuid, node: &str) {
        if let Some(player_perms) = self.attachments.get(player_id) {
            player_perms.remove(node);
        }
    }

    /// 检查是否为玩家显式设置了某项权限。
    #[must_use]
    pub fn has_permission_set(&self, player_id: &uuid::Uuid, node: &str) -> Option<bool> {
        self.attachments
            .get(player_id)
            .and_then(|p| p.get(node).as_deref().copied())
    }

    /// 移除某个玩家的所有权限附件。
    pub fn remove_attachment(&self, player_id: &uuid::Uuid) {
        self.attachments.remove(player_id);
    }

    /// 检索为玩家显式分配的所有权限。
    #[must_use]
    pub fn get_player_permissions(&self, player_id: &uuid::Uuid) -> Option<HashMap<String, bool>> {
        self.attachments.get(player_id).map(|p| {
            p.iter()
                .map(|item| (item.key().clone(), *item.value()))
                .collect()
        })
    }

    /// 检查玩家是否拥有特定权限。
    ///
    /// # Parameters
    /// - `player_id`：玩家的 UUID。
    /// - `permission_node`：要检查的权限节点字符串（例如 "minecraft:command.gamemode"）。
    /// - `player_op_level`：玩家的管理员等级（`PermissionLvl`）。
    ///
    /// # Returns
    /// 如果玩家拥有该权限则为 `true`，否则为 `false`。
    #[must_use]
    pub fn has_permission(
        &self,
        player_id: &uuid::Uuid,
        permission_node: &str,
        player_op_level: PermissionLvl,
    ) -> bool {
        // 检查显式设置的权限
        if let Some(attachment) = self.attachments.get(player_id) {
            // 检查是否存在精确的权限匹配
            if let Some(value) = attachment.get(permission_node) {
                return *value;
            }

            // 检查父节点（用于通配符权限）
            let node_parts: Vec<&str> = permission_node.split(':').collect();
            if node_parts.len() == 2 {
                let namespace = node_parts[0];
                let key_parts: Vec<&str> = node_parts[1].split('.').collect();

                // 在每个级别检查通配符权限
                let mut current_node = namespace.to_string();
                if let Some(value) = attachment.get(&format!("{current_node}:*")) {
                    return *value;
                }

                current_node.push(':');
                for (i, part) in key_parts.iter().enumerate() {
                    current_node.push_str(part);

                    if let Some(value) = attachment.get(&current_node) {
                        return *value;
                    }

                    if i < key_parts.len() - 1 {
                        if let Some(value) = attachment.get(&format!("{current_node}.*")) {
                            return *value;
                        }
                        current_node.push('.');
                    }
                }
            }

            // 检查来自父节点的继承权限
            for item in attachment.iter() {
                let node = item.key();
                let value = *item.value();
                if let Some(permission) = self.registry.get_permission(node)
                    && let Some(child_val) = permission.children.get(permission_node)
                {
                    return value && *child_val;
                }
            }
        }

        // 回退到默认权限值
        self.registry
            .get_permission(permission_node)
            .is_some_and(|permission| match permission.default {
                PermissionDefault::Allow => true,
                PermissionDefault::Deny => false,
                PermissionDefault::Op(required_level) => player_op_level >= required_level,
            })
    }
}

/// 表示玩家的权限等级
///
/// 权限等级决定玩家对命令和服务器操作的访问权限。
/// 每个数字级别对应一个特定的角色：
/// - `Zero`：`normal`：玩家可以使用基础命令。
/// - `One`：`moderator`：玩家可以绕过出生点保护。
/// - `Two`：`gamemaster`：玩家或执行者可以使用更多命令，且玩家可以使用命令方块。
/// - `Three`：`admin`：玩家或执行者可以使用与多人游戏管理相关的命令。
/// - `Four`：`owner`：玩家或执行者可以使用所有命令，包括与服务器管理相关的命令。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PermissionLvl {
    /// 普通玩家。可以使用基本命令。
    #[default]
    Zero = 0,
    /// 协管员。可以绕过出生点保护。
    One = 1,
    /// 游戏管理员。可以使用包括命令方块在内的额外命令。
    Two = 2,
    /// 管理员。可管理多人游戏命令并对玩家进行管理。
    Three = 3,
    /// 所有者。拥有所有命令和服务器管理的完全权限。
    Four = 4,
}

impl PartialOrd for PermissionLvl {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PermissionLvl {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Serialize for PermissionLvl {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for PermissionLvl {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Zero),
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            4 => Ok(Self::Four),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid value for OpLevel: {value}"
            ))),
        }
    }
}
