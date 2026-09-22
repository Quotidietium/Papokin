use heck::ToShoutySnakeCase;
use papokin_util::identifier::Identifier;
use papokin_util::resource_location::ResourceLocation;
use papokin_util::text::TextComponent;
use papokin_util::text::TextContent::Translate;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::PartialEq;
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::{collections::BTreeMap, fs};

/// serde 用于省略时应为 `true` 的字段的辅助默认值。
const fn default_true() -> bool {
    true
}

///包含进度显示信息的结构体
#[derive(Deserialize, Clone)]
pub struct AdvancementDisplay {
    pub title: TextComponent,
    pub description: TextComponent,
    #[serde(rename = "icon", deserialize_with = "deserialize_icon_id")]
    pub item_icon: ResourceLocation,
    #[serde(default, rename = "frame")]
    pub frame_type: FrameTypeStruct,
    #[serde(default, rename = "background")]
    pub background_texture: Option<ResourceLocation>,
    #[serde(default = "default_true")]
    pub show_toast: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default = "default_true")]
    pub announce_to_chat: bool,
    #[serde(skip)]
    pub x: f32,
    #[serde(skip)]
    pub y: f32,
}

fn as_translate(text: &TextComponent) -> TokenStream {
    let Translate { translate, with: _ } = text.0.content.as_ref() else {
        panic!("进度显示应为可翻译的文本组件")
    };
    quote! { #translate }
}

fn token_option<D>(option: &Option<D>) -> TokenStream
where
    D: ToTokens,
{
    match option {
        Some(x) => quote! { Some(#x) },
        None => quote! { None },
    }
}

impl ToTokens for AdvancementDisplay {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let item_icon = format_ident!(
            "{}",
            self.item_icon
                .strip_prefix("minecraft:")
                .unwrap_or_else(|| {
                    panic!("预期为原版 Minecraft 物品图标，实际为 `{}`", self.item_icon)
                })
                .to_uppercase()
        );
        let frame_type = &self.frame_type;
        let announce_to_chat = &self.announce_to_chat;
        let show_toast = &self.show_toast;
        let hidden = &self.hidden;
        let background_texture = token_option(&self.background_texture);
        let title = as_translate(&self.title);
        let description = as_translate(&self.description);
        let x = self.x;
        let y = self.y;
        tokens.extend(quote! {
            AdvancementDisplay::new(#title,
                #description,
                ItemStack::static_new_java(1,&Item::#item_icon),
                #frame_type,
                #background_texture,
                #show_toast,
                #hidden,
                #announce_to_chat,
                #x,
                #y,
            )
        });
    }
}

///存储显示进度时应使用的边框类型
#[derive(Clone, Copy, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FrameTypeStruct {
    #[default]
    Task = 0,
    Challenge = 1,
    Goal = 2,
}

impl ToTokens for FrameTypeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let t = match self {
            FrameTypeStruct::Task => quote! { FrameType::Task },
            FrameTypeStruct::Challenge => quote! { FrameType::Challenge },
            FrameTypeStruct::Goal => quote! { FrameType::Goal },
        };
        tokens.extend(t);
    }
}

///完成进度后给予你的奖励
#[derive(Deserialize, Default, Clone)]
pub struct AdvancementRewards {
    #[serde(default)]
    experience: i32,
    //loot: Vec<ResourceLocation> TODO,
    #[serde(default)]
    recipes: Vec<ResourceLocation>,
    //functions: Option<Function> TODO
}

impl ToTokens for AdvancementRewards {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let experience = self.experience;
        let recipes = self.recipes.iter().map(|_recipe| {
            quote! {
                //TODO 实现配方奖励
                //Recipe::from_id(#recipe)
            }
        });
        tokens.extend(quote! {
            AdvancementReward {
                experience: #experience,
                recipes: &[#(#recipes),*],
            }
        })
    }
}

/// 表示进度树中的一个节点
pub struct AdvancementNode {
    pub children: Vec<usize>,
    pub parent: Option<usize>,
    pub value: AdvancementHolder,
}

impl AdvancementNode {
    #[inline]
    pub fn add_child(&mut self, child: usize) {
        self.children.push(child);
    }

    #[must_use]
    pub fn new(value: AdvancementHolder, parent: Option<usize>) -> Self {
        Self {
            value,
            parent,
            children: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub const fn has_display(&self) -> bool {
        self.value.1.display.is_some()
    }

    #[inline]
    pub const fn set_location(&mut self, x: f32, y: f32) {
        if let Some(val) = self.value.1.display.as_mut() {
            val.x = x;
            val.y = y;
        };
    }
}

impl PartialEq<Self> for AdvancementNode {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for AdvancementNode {}

impl Display for AdvancementNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value.0)
    }
}
impl Hash for AdvancementNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl ToTokens for AdvancementNode {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let parent = token_option(&self.parent);
        let children = &self.children;
        let value = &self.value;
        tokens.extend(quote! {
            AdvancementNode{
                parent:#parent,
                children: vec![#(#children),*],
                value: #value,
            }
        })
    }
}
/// 表示进度树中的一个节点，用于以 Reingold-Tilford 算法计算位置。
///
/// 此结构由定位算法在内部使用，用于计算进度的布局
/// 按客户端显示的样子呈现，与原版 Minecraft 服务器的行为一致。
/// 每个节点存储定位信息、父子关系以及处理过程中使用的临时数据
/// 树遍历算法。
struct TreeNodePosition {
    node: usize,
    parent: Option<usize>,
    previous_sibling: Option<usize>,
    child_index: usize,
    children: Vec<usize>,
    ancestor: usize,
    thread: Option<usize>,
    x: i32,
    y: f32,
    mod_field: f32,
    change: f32,
    shift: f32,
}

impl TreeNodePosition {
    /// 使用 Reingold-Tilford 算法计算并设置树中所有进度节点的 x 和 y 位置。
    ///
    /// 此方法实现三遍式 Reingold-Tilford 算法，以计算最优的层级布局
    /// 用于进度树中的进度节点。该算法确保树在绘制时
    /// 最小宽度，同时保持清晰的父子层级。
    ///
    /// # Arguments
    ///
    /// * `tree` - 包含所有进度节点的 `AdvancementTree` 的可变引用。
    /// 该方法更新每个节点显示信息的 x 和 y 位置。
    /// * `root_index` - 树中根节点的索引，定位算法从此处开始。
    /// 根节点必须具有 display 组件，否则函数将 panic。
    ///
    /// # Panics
    ///
    /// 如果 `root_index` 处的根节点没有显示组件则 panic，因为算法无法
    /// 为不可见节点的子节点定位。
    ///
    /// # Algorithm Overview
    ///
    /// 定位分三个阶段完成：
    /// 1. **第一次遍历**：根据子树放置规则分配初步 x 坐标
    /// 2. **第二次遍历**：将初步坐标转换为最终坐标，并计算最小 y 值
    /// 3. **第三次遍历**：必要时调整所有 y 坐标，以确保其为非负值
    pub fn run(tree: &mut AdvancementTree, root_index: usize) {
        let root_node = if let Some(node) = tree.nodes_vector.get(root_index) {
            node
        } else {
            eprintln!("AdvancementNode 索引越界");
            return;
        };
        if !root_node.has_display() {
            eprintln!("无法定位不可见根节点的子节点！");
            return;
        }
        let mut nodes: Vec<TreeNodePosition> = Vec::with_capacity(32);
        let root_idx = nodes.len();
        nodes.push(TreeNodePosition {
            node: root_index,
            parent: None,
            previous_sibling: None,
            child_index: 1,
            children: Vec::new(),
            ancestor: root_idx,
            thread: None,
            x: 0,
            y: -1.0,
            mod_field: 0.0,
            change: 0.0,
            shift: 0.0,
        });

        let mut previous_idx = None;
        for child in root_node.children.clone() {
            previous_idx = Self::add_child(&mut nodes, tree, root_idx, child, previous_idx);
        }

        Self::first_walk(&mut nodes, root_idx);

        let root_y = nodes[root_idx].y;
        let min = Self::second_walk(&mut nodes, root_idx, 0.0, 0, root_y);

        if min < 0.0 {
            Self::third_walk(&mut nodes, root_idx, -min);
        }

        Self::finalize_position(tree, &nodes, root_idx);
    }

    fn add_child(
        nodes: &mut Vec<TreeNodePosition>,
        tree: &mut AdvancementTree,
        parent_idx: usize,
        adv_node_idx: usize,
        mut previous_idx: Option<usize>,
    ) -> Option<usize> {
        let adv_node = tree.nodes_vector.get(adv_node_idx)?;
        if adv_node.has_display() {
            let child_idx = nodes.len();
            let node = &mut nodes[parent_idx];
            let next_child_index = node.children.len() + 1;
            let depth = node.x + 1;
            node.children.push(child_idx);

            nodes.push(TreeNodePosition {
                node: adv_node_idx,
                parent: Some(parent_idx),
                previous_sibling: previous_idx,
                child_index: next_child_index,
                children: Vec::new(),
                ancestor: child_idx,
                thread: None,
                x: depth,
                y: -1.0,
                mod_field: 0.0,
                change: 0.0,
                shift: 0.0,
            });

            let mut child_prev = None;
            for child in adv_node.children.clone() {
                child_prev = Self::add_child(nodes, tree, child_idx, child, child_prev);
            }

            Some(child_idx)
        } else {
            for grandchild in &adv_node.children.clone() {
                previous_idx = Self::add_child(nodes, tree, parent_idx, *grandchild, previous_idx);
            }
            previous_idx
        }
    }

    /// 树定位算法的首次遍历。
    ///
    /// 此函数根据子树放置规则为节点分配初步 y 坐标。
    /// 它执行后序遍历（先子节点后父节点）来递归计算位置。
    ///
    /// 对于叶子节点，y 坐标基于前一个兄弟节点的位置设置。
    /// 对于有子节点的节点，y 坐标按以下位置之间的中点计算：
    /// 在应用均分（apportion）算法解决重叠后的第一个与最后一个子节点。
    ///
    /// # Arguments
    ///
    /// * `nodes` - 表示树结构的 `TreeNodePosition` 向量的可变引用。
    /// * `idx` - 当前正在处理的节点在 `nodes` 向量中的索引。
    ///
    /// # Algorithm Details
    ///
    /// - 先递归处理所有子节点（后序遍历）
    /// - 使用 `apportion()` 检测并解决子树间的重叠
    /// - 执行移位操作，将定位调整沿树向下传播
    /// - 将节点的 y 坐标计算为子节点的中点，或者定位为
    ///   相对于前一个兄弟节点
    ///
    /// # Note
    ///
    /// 这是计算最终位置所需三次遍历中的第一次。它设置初步的
    /// 坐标，后续各轮处理会进一步细化。
    fn first_walk(nodes: &mut Vec<TreeNodePosition>, idx: usize) {
        let num_children = nodes[idx].children.len();
        if num_children == 0 {
            if let Some(prev_sib) = nodes[idx].previous_sibling {
                nodes[idx].y = nodes[prev_sib].y + 1.0;
            } else {
                nodes[idx].y = 0.0;
            }
        } else {
            let mut default_ancestor: Option<usize> = None;
            for i in 0..num_children {
                let child_idx = nodes[idx].children[i];
                Self::first_walk(nodes, child_idx);
                let arg_ancestor = default_ancestor.unwrap_or(child_idx);
                default_ancestor = Some(Self::apportion(nodes, child_idx, arg_ancestor));
            }

            Self::execute_shifts(nodes, idx);

            let node = &mut nodes[idx];
            let first_child_idx = node.children[0];
            let last_child_idx = node.children[num_children - 1];
            let midpoint = (nodes[first_child_idx].y + nodes[last_child_idx].y) / 2.0;

            if let Some(prev_sib) = nodes[idx].previous_sibling {
                nodes[idx].y = nodes[prev_sib].y + 1.0;
                nodes[idx].mod_field = nodes[idx].y - midpoint;
            } else {
                nodes[idx].y = midpoint;
            }
        }
    }

    /// 树木定位算法的第二次遍历。
    ///
    /// 此函数将初步坐标转换为最终坐标，并计算
    /// 遍历中遇到的最小 y 值。它执行先序遍历
    /// (父节点在前、子节点在后)，以便将修改从父节点累积到子节点。
    ///
    /// # Arguments
    ///
    /// * `nodes` - 表示树结构的 `TreeNodePosition` 向量的可变引用。
    /// * `idx` - 当前正在处理的节点在 `nodes` 向量中的索引。
    /// * `mod_sum` - 从所有祖先节点累积的修改偏移量。该值
    /// 相加，将初步坐标转换为最终坐标。
    /// * `depth` - 当前节点在树中的深度（根节点为 0，子节点依次递增）。
    /// * `mut min` - 遍历到目前为止遇到的最小 y 坐标。
    ///
    /// # Returns
    ///
    /// 当前节点及其所有后代节点中的最小 y 坐标值。
    ///
    /// # Algorithm Details
    ///
    /// - 应用累积的修改，将初步 y 坐标转换为最终坐标
    /// - 设置用于水平排布节点的 x 坐标（深度）
    /// - 跟踪最小 y 值以检测是否需要调整
    /// - 递归处理所有子节点，并累加它们的修饰符偏移
    ///
    /// # Note
    ///
    /// 这是三次遍历中的第二次。返回的最小值用于确保
    /// 第三次遍历时所有 y 坐标均为非负。
    fn second_walk(
        nodes: &mut Vec<TreeNodePosition>,
        idx: usize,
        mod_sum: f32,
        depth: i32,
        mut min: f32,
    ) -> f32 {
        let node = &mut nodes[idx];
        node.y += mod_sum;
        node.x = depth;

        if node.y < min {
            min = node.y;
        }

        let num_children = node.children.len();
        let current_mod = node.mod_field;

        for i in 0..num_children {
            let child_idx = nodes[idx].children[i];
            min = Self::second_walk(nodes, child_idx, mod_sum + current_mod, depth + 1, min);
        }

        min
    }

    /// 树定位算法的第三次遍历。
    ///
    /// 此函数通过添加统一偏移量来调整树的所有 y 坐标，从而确保
    /// 所有坐标均为非负。它递归地遍历树，并应用相同的
    /// 偏移量应用到每个节点及其后代。
    ///
    /// # Arguments
    ///
    /// * `nodes` - 表示树结构的 `TreeNodePosition` 向量的可变引用。
    /// * `idx` - 当前正在处理的节点在 `nodes` 向量中的索引。
    /// * `offset` - 要应用的 y 坐标偏移量。通常为最小值的相反数
    /// 第二次遍历中找到的 y 值。
    ///
    /// # Algorithm Details
    ///
    /// - 将偏移量加到当前节点的 y 坐标上
    /// - 递归地对所有子节点应用相同的偏移
    /// - 使用简单的后序遍历确保整棵树得到均匀调整
    ///
    /// # Note
    ///
    /// 这是三次遍历中的第三次。仅当此前遍历中找到的最小 y 值满足条件时才执行
    /// 第二次游走为负数，确保所有最终位置均为非负。
    fn third_walk(nodes: &mut Vec<TreeNodePosition>, _idx: usize, offset: f32) {
        nodes.iter_mut().for_each(|node| {
            node.y += offset;
        });
    }

    fn execute_shifts(nodes: &mut [TreeNodePosition], idx: usize) {
        let mut shift = 0.0;
        let mut change = 0.0;

        for &child_idx in nodes[idx].children.iter().rev() {
            nodes[child_idx].y += shift;
            nodes[child_idx].mod_field += shift;
            change += nodes[child_idx].change;
            shift += nodes[child_idx].shift + change;
        }
    }

    #[inline]
    fn previous_or_thread(nodes: &[TreeNodePosition], idx: usize) -> Option<usize> {
        nodes[idx]
            .thread
            .or_else(|| nodes[idx].children.first().copied())
    }

    #[inline]
    fn next_or_thread(nodes: &[TreeNodePosition], idx: usize) -> Option<usize> {
        nodes[idx]
            .thread
            .or_else(|| nodes[idx].children.last().copied())
    }

    fn apportion(nodes: &mut [TreeNodePosition], idx: usize, mut default_ancestor: usize) -> usize {
        let prev_sib = match nodes[idx].previous_sibling {
            Some(p) => p,
            None => return default_ancestor,
        };
        let parent_idx = nodes[idx].parent.expect("树不变量被破坏：缺少父节点");
        let mut inner_right = idx;
        let mut outer_right = idx;
        let mut inner_left = prev_sib;
        let mut outer_left = nodes[parent_idx].children[0];

        let mod_field = nodes[idx].mod_field;
        let mut shift_inner_right = mod_field;
        let mut shift_outer_right = mod_field;
        let mut shift_inner_left = nodes[inner_left].mod_field;
        let mut shift_outer_left = nodes[outer_left].mod_field;
        while let Some(next_inner_left) = Self::next_or_thread(nodes, inner_left)
            && let Some(next_inner_right) = Self::previous_or_thread(nodes, inner_right)
        {
            inner_left = next_inner_left;
            inner_right = next_inner_right;
            outer_left = Self::previous_or_thread(nodes, outer_left).expect("树不变量被破坏");
            outer_right = Self::next_or_thread(nodes, outer_right).expect("树不变量被破坏");

            nodes[outer_right].ancestor = idx;

            let shift = (nodes[inner_left].y + shift_inner_left)
                - (nodes[inner_right].y + shift_inner_right)
                + 1.0;
            if shift > 0.0 {
                let ancestor_idx = Self::get_ancestor(nodes, inner_left, idx, default_ancestor);
                Self::move_subtree(nodes, ancestor_idx, idx, shift);
                shift_inner_right += shift;
                shift_outer_right += shift;
            }

            shift_inner_left += nodes[inner_left].mod_field;
            shift_inner_right += nodes[inner_right].mod_field;
            shift_outer_left += nodes[outer_left].mod_field;
        }

        if let Some(next_inner_left) = Self::next_or_thread(nodes, inner_left)
            && Self::next_or_thread(nodes, outer_right).is_none()
        {
            nodes[outer_right].thread = Some(next_inner_left);
            nodes[outer_right].mod_field += shift_inner_left - shift_outer_right;
        } else {
            if let Some(next_inner_right) = Self::previous_or_thread(nodes, inner_right)
                && Self::previous_or_thread(nodes, outer_left).is_none()
            {
                nodes[outer_left].thread = Some(next_inner_right);
                nodes[outer_left].mod_field += shift_inner_right - shift_outer_left;
            }
            default_ancestor = idx;
        }
        default_ancestor
    }

    fn move_subtree(nodes: &mut [TreeNodePosition], left: usize, right: usize, shift: f32) {
        let subtrees = (nodes[right].child_index as f32) - (nodes[left].child_index as f32);
        if subtrees != 0.0 {
            nodes[right].change -= shift / subtrees;
            nodes[left].change += shift / subtrees;
        }
        nodes[right].shift += shift;
        nodes[right].y += shift;
        nodes[right].mod_field += shift;
    }

    fn get_ancestor(
        nodes: &[TreeNodePosition],
        idx: usize,
        other: usize,
        default_ancestor: usize,
    ) -> usize {
        let ancestor = nodes[idx].ancestor;
        let parent_idx = nodes[other].parent.unwrap();

        if nodes[parent_idx].children.contains(&ancestor) {
            ancestor
        } else {
            default_ancestor
        }
    }

    /// 树定位算法的最后一轮遍历。
    ///
    /// 此函数将计算出的位置应用到树中实际的进度节点上，
    /// 最终确定它们的显示位置。它会递归遍历树并更新每个节点的
    /// 树结构中的位置坐标。
    ///
    /// # Arguments
    ///
    /// * `tree` - `AdvancementTree` 的可变引用。该树将被更新
    ///   由 `TreeNodePosition` 节点计算出的 x 和 y 位置。
    /// * `nodes` - 包含已计算位置的 `TreeNodePosition` 向量的引用
    ///   对树中的每个节点执行。
    /// * `idx` - 当前正在处理的节点在 `nodes` 向量中的索引。
    ///
    /// # Algorithm Details
    ///
    /// - 从给定索引处的 `TreeNodePosition` 获取计算出的 x 和 y 位置
    /// - 将这些位置设置到树中对应的进度节点上
    /// - 递归处理所有子节点，同时更新它们的位置
    /// - 使用后序遍历确保所有节点位置正确
    ///
    /// # Note
    ///
    /// 这是第四次也是最后一次遍历。只应在三次定位遍历全部完成后才调用它
    /// (第一、第二和第三步) 已全部成功完成。此函数会转移
    /// 把内部 `TreeNodePosition` 结构算得的位置转换回实际的
    /// `AdvancementNode` 的显示信息。
    fn finalize_position(tree: &mut AdvancementTree, nodes: &[TreeNodePosition], idx: usize) {
        tree.nodes_vector[nodes[idx].node].set_location(nodes[idx].x as f32, nodes[idx].y);
        for &child_idx in &nodes[idx].children {
            Self::finalize_position(tree, nodes, child_idx);
        }
    }
}

/// 表示进度的结构体
#[derive(Deserialize, Default, Clone)]
pub struct AdvancementStruct {
    pub parent: Option<Identifier>,
    #[serde(default)]
    pub display: Option<AdvancementDisplay>,
    //pub criteria : Vec<Criterion>,
    #[serde(default)]
    pub rewards: AdvancementRewards,
    #[serde(default, rename = "sends_telemetry_event")]
    pub sends_telemetry: bool,
    pub requirements: Vec<Vec<String>>,
    #[serde(deserialize_with = "deserialize_first_key")]
    pub criteria: Vec<String>,
}

fn deserialize_first_key<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let map = BTreeMap::<String, serde_json::Value>::deserialize(deserializer)?;
    Ok(map.into_keys().collect())
}

#[derive(Clone)]
pub struct AdvancementHolder(Identifier, AdvancementStruct);

impl PartialEq for AdvancementHolder {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl Eq for AdvancementHolder {}

impl Hash for AdvancementHolder {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl ToTokens for AdvancementHolder {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = format_ident!("{}", self.0.path().to_shouty_snake_case());
        tokens.extend(quote! {
            Advancement::#name
        })
    }
}

///用作显示图标的物品
///
///（不支持自定义物品，因为原版进度不使用自定义物品）
#[derive(Deserialize)]
struct DisplayIcon {
    id: ResourceLocation,
}

fn deserialize_icon_id<'de, D>(deserializer: D) -> Result<ResourceLocation, D::Error>
where
    D: Deserializer<'de>,
{
    let icon = DisplayIcon::deserialize(deserializer)?;
    Ok(icon.id)
}

/// 表示用于存储各节点并将其 id 关联到对应节点的结构。
#[derive(Default)]
pub struct AdvancementTree {
    pub nodes: BTreeMap<Identifier, usize>,
    pub nodes_vector: Vec<AdvancementNode>,
    pub roots: Vec<usize>,
    pub tasks: Vec<usize>,
}

impl AdvancementTree {
    ///遍历所有进度，直到每个可插入的进度都被插入。
    ///
    ///参见 [`AdvancementTree::try_insert`]
    pub fn add_all(&mut self, advancements: Vec<AdvancementHolder>) {
        let mut advancements_to_add: Vec<AdvancementHolder> = advancements;

        while !advancements_to_add.is_empty() {
            let len_before = advancements_to_add.len();

            advancements_to_add = advancements_to_add
                .into_iter()
                .filter_map(|advancement| self.try_insert(advancement))
                .collect();

            if advancements_to_add.len() == len_before && !advancements_to_add.is_empty() {
                eprintln!(
                    "无法加载进度：{:?}",
                    advancements_to_add.iter().map(|a| &a.0).collect::<Vec<_>>()
                );
                break;
            }
        }
    }

    ///如果该进度拥有父进度且尚未注册，则尝试将其插入进度树中
    /// 并返回具有所有权的 AdvancementHolder
    pub fn try_insert(&mut self, advancement: AdvancementHolder) -> Option<AdvancementHolder> {
        let parent_id = &advancement.1.parent;
        let parent_idx: Option<usize> = match parent_id {
            Some(id) => match self.nodes.get(id) {
                Some(node) => Some(*node),
                None => return Some(advancement),
            },
            None => None,
        };
        let id = advancement.0.clone();
        let node = AdvancementNode::new(advancement, parent_idx);
        let node_idx = self.nodes_vector.len();
        self.nodes.insert(id, node_idx);
        if let Some(parent) = parent_idx {
            let parent_node = self.nodes_vector.get_mut(parent).unwrap();
            parent_node.add_child(node_idx);
            self.tasks.push(node_idx);
        } else {
            self.roots.push(node_idx);
        }
        self.nodes_vector.push(node);
        None
    }
}

impl ToTokens for AdvancementTree {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let nodes = self.nodes.iter().map(|(k, v)| {
            let key = identifier_to_tokens(k);
            quote! {
                nodes.insert(#key, #v);
            }
        });
        let nodes_vector = &self.nodes_vector;
        let roots = &self.roots;
        let tasks = &self.tasks;
        tokens.extend(quote! {
            LazyLock::new(|| {
                let mut nodes = BTreeMap::new();
                #(#nodes)*
                let nodes_vector = vec![#(#nodes_vector),*];
                let roots = vec![#(#roots),*];
                let tasks = vec![#(#tasks),*];
                AdvancementTree {
                    nodes,
                    nodes_vector,
                    roots,
                    tasks,
                }
            })
        })
    }
}

///将标识符转换为其令牌形式
fn identifier_to_tokens(identifier: &Identifier) -> TokenStream {
    let namespace = identifier.namespace();
    let path = identifier.path();
    quote! {
        Identifier::from_static(#namespace, #path)
    }
}

fn collect_advancements(
    base: &std::path::Path,
    dir: &std::path::Path,
    result: &mut BTreeMap<String, AdvancementStruct>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_advancements(base, &path, result);
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let rel = path.strip_prefix(base).unwrap();
            let rel_str = rel
                .with_extension("")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let key = format!("minecraft:{rel_str}");
            if let Ok(content) = fs::read_to_string(&path)
                && let Ok(adv) = serde_json::from_str::<AdvancementStruct>(&content)
            {
                result.insert(key, adv);
            }
        }
    }
}

/// 进度代码生成的入口点。
///
/// 解析 26.2 数据包中的进度文件，构建进度树，
/// 使用 Reingold-Tilford 算法计算位置，并生成
/// 最终的 Rust 源代码。
pub(crate) fn build() -> TokenStream {
    let base_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/datapacks/26_3/data/minecraft/advancement");
    let mut advancements: BTreeMap<String, AdvancementStruct> = BTreeMap::new();
    collect_advancements(&base_path, &base_path, &mut advancements);

    let mut variants = TokenStream::new();
    let mut name_to_type = TokenStream::new();
    let mut minecraft_name_to_type = TokenStream::new();
    let mut minecraft_namespaces = TokenStream::new();
    let mut advancement_list = TokenStream::new();
    let capacity = advancements.len();
    //构建树
    let mut tree = AdvancementTree::default();
    tree.add_all(
        advancements
            .into_iter()
            .map(|(key, value)| AdvancementHolder(Identifier::parse(&key).unwrap(), value))
            .collect(),
    );
    for index in tree.roots.clone() {
        if tree.nodes_vector.get(index).unwrap().has_display() {
            TreeNodePosition::run(&mut tree, index);
        }
    }
    let advancement_tree = quote! {
        pub static ADVANCEMENT_TREE : LazyLock<AdvancementTree> = #tree;
    };
    let advancements_holder = tree.nodes_vector.into_iter().map(|node| node.value);
    for AdvancementHolder(identifier, advancement) in advancements_holder {
        let raw_name = identifier.path();
        let format_name = format_ident!("{}", raw_name.to_shouty_snake_case());

        let parent = if let Some(identifier) = &advancement.parent {
            let parent = identifier_to_tokens(identifier);
            quote! {Some(#parent)}
        } else {
            quote! { None }
        };
        let send_telemetry = advancement.sends_telemetry;
        let display = match &advancement.display {
            Some(display) => quote! { Some(&#display) },
            None => quote! { None },
        };
        let reward = advancement.rewards;
        let requirements = advancement.requirements.iter().map(|inner_req| {
            quote! { &[#(#inner_req),*]}
        });
        let criteria = advancement.criteria;
        variants.extend([quote! {
            pub const #format_name: &Self = &Self {
                id: Identifier::vanilla_static(#raw_name),
                parent : #parent,
                send_telemetry : #send_telemetry,
                display : #display,
                reward : &#reward,
                requirements: &[#(#requirements),*],
                criteria: &[#(#criteria),*],
            };
        }]);
        let minecraft_name = identifier.to_string();

        name_to_type.extend(quote! { #raw_name => Some(Self::#format_name), });
        minecraft_name_to_type.extend(quote! { #minecraft_name => Some(Self::#format_name), });
        minecraft_namespaces.extend(quote! { Identifier::vanilla_static(#raw_name),});
        advancement_list.extend(quote! {Self::#format_name, });
    }

    quote! {
        use papokin_util::text::TextComponent;
        use crate::item_stack::ItemStack;
        use crate::item::Item;
        use crate::advancement_data::*;
        use std::sync::LazyLock;
        use papokin_util::identifier::Identifier;
        use papokin_util::text::{color::NamedColor,
            style::Style,
            hover::HoverEvent,
            color::Color};
        use std::hash::{Hash,Hasher};
        use std::fmt::Display;
        use std::collections::BTreeMap;

        pub struct Advancement {
            pub id : Identifier,
            pub parent : Option<Identifier>,
            pub send_telemetry : bool,
            pub display : Option<&'static AdvancementDisplay>,
            pub reward : &'static AdvancementReward,
            pub requirements: &'static[&'static[&'static str]],
            pub criteria: &'static[&'static str],
        }

        impl Display for Advancement {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.id)
            }
        }

        impl Hash for Advancement {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        impl PartialEq<Self> for Advancement {
            fn eq(&self, other: &Self) -> bool {
                other.id == self.id
            }
        }

        impl Eq for Advancement {}

        impl Advancement {
            #variants

            pub fn option_name(&self) -> Option<TextComponent> {
                match self.display {
                    Some(display) => {
                        let mut over = display.get_title();
                        let color = Color::Named(display.frame_type.get_color());
                        *over.0.style = Style::default().color(color);
                        over = over.add_text("\n").add_child(display.get_description());
                        let mut text = display.get_title();
                        text.0.style.hover_event = Some(HoverEvent::show_text(over));
                        Some(text.wrap_in_square_brackets().color(color))
                    }
                    None => None
                }
            }

            pub fn name(&self) -> TextComponent {
                self.option_name().unwrap_or(TextComponent::text(self.id.to_string()))
            }

            pub fn from_name(name: &str) -> Option<&'static Self> {
                    match name {
                        #name_to_type
                        _ => None
                    }
                }


            pub fn from_minecraft_name(name: &str) -> Option<&'static Self> {
                match name {
                    #minecraft_name_to_type
                    _ => None
                }
            }

            pub fn get_advancements_list() -> [&'static Advancement; #capacity] {
                [#advancement_list]
            }

            pub const fn get_identifier_list() -> [Identifier;#capacity] {
                [#minecraft_namespaces]
            }

            pub const fn is_root(&self) -> bool{
                self.parent.is_none()
            }

        }
        #advancement_tree
    }
}
