// ImNodeFlow C++ wrapper for dear-imgui-sys compatibility
// This file includes ImNodeFlow sources and provides C++ bindings

// Define required macros before including headers
#define IMGUI_DEFINE_MATH_OPERATORS

// Include Dear ImGui headers only (implementation comes from dear-imgui-sys)
#include "imgui.h"
#include "imgui_internal.h"

// Include ImNodeFlow implementation
#include "ImNodeFlow.h"
#include "ImNodeFlow.cpp"

// C-style wrapper functions
extern "C" {
    // ImNodeFlow wrapper functions
    ImFlow::ImNodeFlow* ImNodeFlow_Create(const char* name) {
        return new ImFlow::ImNodeFlow(std::string(name));
    }

    ImFlow::ImNodeFlow* ImNodeFlow_CreateDefault() {
        return new ImFlow::ImNodeFlow();
    }

    void ImNodeFlow_Destroy(ImFlow::ImNodeFlow* inf) {
        delete inf;
    }

    void ImNodeFlow_Update(ImFlow::ImNodeFlow* inf) {
        inf->update();
    }

    void ImNodeFlow_SetSize(ImFlow::ImNodeFlow* inf, float width, float height) {
        inf->setSize(ImVec2(width, height));
    }

    const char* ImNodeFlow_GetName(ImFlow::ImNodeFlow* inf) {
        static std::string name_copy;
        name_copy = inf->getName();
        return name_copy.c_str();
    }

    ImVec2 ImNodeFlow_GetPos(ImFlow::ImNodeFlow* inf) {
        return inf->getPos();
    }

    ImVec2 ImNodeFlow_GetScroll(ImFlow::ImNodeFlow* inf) {
        return inf->getScroll();
    }

    uint32_t ImNodeFlow_GetNodesCount(ImFlow::ImNodeFlow* inf) {
        return inf->getNodesCount();
    }

    bool ImNodeFlow_IsNodeDragged(ImFlow::ImNodeFlow* inf) {
        return inf->isNodeDragged();
    }

    bool ImNodeFlow_GetSingleUseClick(ImFlow::ImNodeFlow* inf) {
        return inf->getSingleUseClick();
    }

    void ImNodeFlow_ConsumeSingleUseClick(ImFlow::ImNodeFlow* inf) {
        inf->consumeSingleUseClick();
    }

    ImVec2 ImNodeFlow_Screen2Grid(ImFlow::ImNodeFlow* inf, ImVec2 p) {
        return inf->screen2grid(p);
    }

    ImVec2 ImNodeFlow_Grid2Screen(ImFlow::ImNodeFlow* inf, ImVec2 p) {
        return inf->grid2screen(p);
    }

    bool ImNodeFlow_OnSelectedNode(ImFlow::ImNodeFlow* inf) {
        return inf->on_selected_node();
    }

    bool ImNodeFlow_OnFreeSpace(ImFlow::ImNodeFlow* inf) {
        return inf->on_free_space();
    }

    void ImNodeFlow_DraggingNode(ImFlow::ImNodeFlow* inf, bool state) {
        inf->draggingNode(state);
    }

    void ImNodeFlow_Hovering(ImFlow::ImNodeFlow* inf, ImFlow::Pin* pin) {
        inf->hovering(pin);
    }

    void ImNodeFlow_HoveredNode(ImFlow::ImNodeFlow* inf, ImFlow::BaseNode* node) {
        inf->hoveredNode(node);
    }

    // BaseNode wrapper functions
    ImFlow::BaseNode* BaseNode_Create() {
        return new ImFlow::BaseNode();
    }

    void BaseNode_Destroy(ImFlow::BaseNode* node) {
        delete node;
    }

    void BaseNode_Update(ImFlow::BaseNode* node) {
        node->update();
    }

    void BaseNode_SetTitle(ImFlow::BaseNode* node, const char* title) {
        node->setTitle(std::string(title));
    }

    void BaseNode_SetPos(ImFlow::BaseNode* node, float x, float y) {
        node->setPos(ImVec2(x, y));
    }

    void BaseNode_SetHandler(ImFlow::BaseNode* node, ImFlow::ImNodeFlow* inf) {
        node->setHandler(inf);
    }

    void BaseNode_SetStyle(ImFlow::BaseNode* node, std::shared_ptr<ImFlow::NodeStyle>* style) {
        node->setStyle(*style);
    }

    void BaseNode_Selected(ImFlow::BaseNode* node, bool state) {
        node->selected(state);
    }

    void BaseNode_UpdatePublicStatus(ImFlow::BaseNode* node) {
        node->updatePublicStatus();
    }

    void BaseNode_Destroy_Node(ImFlow::BaseNode* node) {
        node->destroy();
    }

    bool BaseNode_ToDestroy(ImFlow::BaseNode* node) {
        return node->toDestroy();
    }

    bool BaseNode_IsHovered(ImFlow::BaseNode* node) {
        return node->isHovered();
    }

    bool BaseNode_IsSelected(ImFlow::BaseNode* node) {
        return node->isSelected();
    }

    bool BaseNode_IsDragged(ImFlow::BaseNode* node) {
        return node->isDragged();
    }

    ImFlow::NodeUID BaseNode_GetUID(ImFlow::BaseNode* node) {
        return node->getUID();
    }

    const char* BaseNode_GetName(ImFlow::BaseNode* node) {
        static std::string name_copy;
        name_copy = node->getName();
        return name_copy.c_str();
    }

    ImVec2 BaseNode_GetSize(ImFlow::BaseNode* node) {
        return node->getSize();
    }

    ImVec2 BaseNode_GetPos(ImFlow::BaseNode* node) {
        return node->getPos();
    }

    ImFlow::ImNodeFlow* BaseNode_GetHandler(ImFlow::BaseNode* node) {
        return node->getHandler();
    }

    std::shared_ptr<ImFlow::NodeStyle>* BaseNode_GetStyle(ImFlow::BaseNode* node) {
        static std::shared_ptr<ImFlow::NodeStyle> style_copy;
        style_copy = node->getStyle();
        return &style_copy;
    }

    // Pin wrapper functions
    ImFlow::PinUID Pin_GetUid(ImFlow::Pin* pin) {
        return pin->getUid();
    }

    const char* Pin_GetName(ImFlow::Pin* pin) {
        static std::string name_copy;
        name_copy = pin->getName();
        return name_copy.c_str();
    }

    ImVec2 Pin_GetPos(ImFlow::Pin* pin) {
        return pin->getPos();
    }

    ImVec2 Pin_GetSize(ImFlow::Pin* pin) {
        return pin->getSize();
    }

    ImFlow::BaseNode* Pin_GetParent(ImFlow::Pin* pin) {
        return pin->getParent();
    }

    int Pin_GetType(ImFlow::Pin* pin) {
        return static_cast<int>(pin->getType());
    }

    std::shared_ptr<ImFlow::PinStyle>* Pin_GetStyle(ImFlow::Pin* pin) {
        static std::shared_ptr<ImFlow::PinStyle> style_copy;
        style_copy = pin->getStyle();
        return &style_copy;
    }

    ImVec2 Pin_PinPoint(ImFlow::Pin* pin) {
        return pin->pinPoint();
    }

    float Pin_CalcWidth(ImFlow::Pin* pin) {
        return pin->calcWidth();
    }

    void Pin_SetPos(ImFlow::Pin* pin, float x, float y) {
        pin->setPos(ImVec2(x, y));
    }

    bool Pin_IsConnected(ImFlow::Pin* pin) {
        return pin->isConnected();
    }

    void Pin_CreateLink(ImFlow::Pin* pin, ImFlow::Pin* other) {
        pin->createLink(other);
    }

    void Pin_DeleteLink(ImFlow::Pin* pin) {
        pin->deleteLink();
    }

    // Link wrapper functions
    ImFlow::Link* Link_Create(ImFlow::Pin* left, ImFlow::Pin* right, ImFlow::ImNodeFlow* inf) {
        return new ImFlow::Link(left, right, inf);
    }

    void Link_Destroy(ImFlow::Link* link) {
        delete link;
    }

    void Link_Update(ImFlow::Link* link) {
        link->update();
    }

    ImFlow::Pin* Link_Left(ImFlow::Link* link) {
        return link->left();
    }

    ImFlow::Pin* Link_Right(ImFlow::Link* link) {
        return link->right();
    }

    bool Link_IsHovered(ImFlow::Link* link) {
        return link->isHovered();
    }

    bool Link_IsSelected(ImFlow::Link* link) {
        return link->isSelected();
    }

    // PinStyle wrapper functions
    std::shared_ptr<ImFlow::PinStyle>* PinStyle_Create(ImU32 color, int socket_shape, 
                                                      float socket_radius, float socket_hovered_radius, 
                                                      float socket_connected_radius, float socket_thickness) {
        static std::shared_ptr<ImFlow::PinStyle> style;
        style = std::make_shared<ImFlow::PinStyle>(color, socket_shape, socket_radius, 
                                                  socket_hovered_radius, socket_connected_radius, 
                                                  socket_thickness);
        return &style;
    }

    std::shared_ptr<ImFlow::PinStyle>* PinStyle_Cyan() {
        static std::shared_ptr<ImFlow::PinStyle> style = ImFlow::PinStyle::cyan();
        return &style;
    }

    std::shared_ptr<ImFlow::PinStyle>* PinStyle_Green() {
        static std::shared_ptr<ImFlow::PinStyle> style = ImFlow::PinStyle::green();
        return &style;
    }

    std::shared_ptr<ImFlow::PinStyle>* PinStyle_Blue() {
        static std::shared_ptr<ImFlow::PinStyle> style = ImFlow::PinStyle::blue();
        return &style;
    }

    std::shared_ptr<ImFlow::PinStyle>* PinStyle_Brown() {
        static std::shared_ptr<ImFlow::PinStyle> style = ImFlow::PinStyle::brown();
        return &style;
    }

    std::shared_ptr<ImFlow::PinStyle>* PinStyle_Red() {
        static std::shared_ptr<ImFlow::PinStyle> style = ImFlow::PinStyle::red();
        return &style;
    }

    std::shared_ptr<ImFlow::PinStyle>* PinStyle_White() {
        static std::shared_ptr<ImFlow::PinStyle> style = ImFlow::PinStyle::white();
        return &style;
    }

    void PinStyle_Destroy(std::shared_ptr<ImFlow::PinStyle>* style) {
        // Shared pointers handle their own destruction
    }

    // NodeStyle wrapper functions
    std::shared_ptr<ImFlow::NodeStyle>* NodeStyle_Create(ImU32 header_bg, ImU32 header_title_color, float radius) {
        static std::shared_ptr<ImFlow::NodeStyle> style;
        style = std::make_shared<ImFlow::NodeStyle>(header_bg, ImColor(header_title_color), radius);
        return &style;
    }

    std::shared_ptr<ImFlow::NodeStyle>* NodeStyle_Cyan() {
        static std::shared_ptr<ImFlow::NodeStyle> style = ImFlow::NodeStyle::cyan();
        return &style;
    }

    std::shared_ptr<ImFlow::NodeStyle>* NodeStyle_Green() {
        static std::shared_ptr<ImFlow::NodeStyle> style = ImFlow::NodeStyle::green();
        return &style;
    }

    std::shared_ptr<ImFlow::NodeStyle>* NodeStyle_Red() {
        static std::shared_ptr<ImFlow::NodeStyle> style = ImFlow::NodeStyle::red();
        return &style;
    }

    std::shared_ptr<ImFlow::NodeStyle>* NodeStyle_Brown() {
        static std::shared_ptr<ImFlow::NodeStyle> style = ImFlow::NodeStyle::brown();
        return &style;
    }

    void NodeStyle_Destroy(std::shared_ptr<ImFlow::NodeStyle>* style) {
        // Shared pointers handle their own destruction
    }

    // Node creation functions - we'll create specific node types
    ImFlow::BaseNode* ImNodeFlow_AddSimpleNode(ImFlow::ImNodeFlow* inf, float x, float y, const char* title) {
        // Create a simple node with basic functionality
        class SimpleNode : public ImFlow::BaseNode {
        public:
            SimpleNode(const std::string& title) {
                setTitle(title);
                setStyle(ImFlow::NodeStyle::green());
            }
            void draw() override {
                // Basic node content - can be customized later
            }
        };

        auto node = std::make_shared<SimpleNode>(std::string(title));
        node->setPos(ImVec2(x, y));
        node->setHandler(inf);

        // Add to the node flow's internal map
        auto& nodes = inf->getNodes();
        nodes[node->getUID()] = node;

        return node.get();
    }

    // Pin creation functions for BaseNode
    ImFlow::Pin* BaseNode_AddInputPin(ImFlow::BaseNode* node, const char* name, int data_type) {
        // For now, we'll create a generic input pin
        // In a full implementation, this would be templated
        if (data_type == 0) { // int type
            auto pin = node->addIN<int>(std::string(name), 0, ImFlow::ConnectionFilter::SameType());
            return pin.get();
        }
        return nullptr;
    }

    ImFlow::Pin* BaseNode_AddOutputPin(ImFlow::BaseNode* node, const char* name, int data_type) {
        // For now, we'll create a generic output pin
        if (data_type == 0) { // int type
            auto pin = node->addOUT<int>(std::string(name), nullptr);
            return pin.get();
        }
        return nullptr;
    }

    // Get pin by name
    ImFlow::Pin* BaseNode_GetInputPin(ImFlow::BaseNode* node, const char* name) {
        return node->inPin(std::string(name));
    }

    ImFlow::Pin* BaseNode_GetOutputPin(ImFlow::BaseNode* node, const char* name) {
        return node->outPin(std::string(name));
    }

    // Helper functions
    void ImNodeFlow_SmartBezier(ImVec2 p1, ImVec2 p2, ImU32 color, float thickness) {
        ImFlow::smart_bezier(p1, p2, color, thickness);
    }

    bool ImNodeFlow_SmartBezierCollider(ImVec2 p, ImVec2 p1, ImVec2 p2, float radius) {
        return ImFlow::smart_bezier_collider(p, p1, p2, radius);
    }
}
