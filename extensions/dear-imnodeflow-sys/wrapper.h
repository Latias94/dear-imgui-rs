#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Forward declarations for ImGui types
typedef struct ImVec2 { float x, y; } ImVec2;
typedef struct ImVec4 { float x, y, z, w; } ImVec4;
typedef uint32_t ImU32;
typedef void* ImNodeFlowPtr;
typedef void* BaseNodePtr;
typedef void* PinPtr;
typedef void* LinkPtr;
typedef void* PinStylePtr;
typedef void* NodeStylePtr;
typedef unsigned long long int PinUID;
typedef uintptr_t NodeUID;

// ImNodeFlow wrapper functions
ImNodeFlowPtr ImNodeFlow_Create(const char* name);
ImNodeFlowPtr ImNodeFlow_CreateDefault();
void ImNodeFlow_Destroy(ImNodeFlowPtr inf);
void ImNodeFlow_Update(ImNodeFlowPtr inf);
void ImNodeFlow_SetSize(ImNodeFlowPtr inf, float width, float height);
const char* ImNodeFlow_GetName(ImNodeFlowPtr inf);
ImVec2 ImNodeFlow_GetPos(ImNodeFlowPtr inf);
ImVec2 ImNodeFlow_GetScroll(ImNodeFlowPtr inf);
uint32_t ImNodeFlow_GetNodesCount(ImNodeFlowPtr inf);
bool ImNodeFlow_IsNodeDragged(ImNodeFlowPtr inf);
bool ImNodeFlow_GetSingleUseClick(ImNodeFlowPtr inf);
void ImNodeFlow_ConsumeSingleUseClick(ImNodeFlowPtr inf);
ImVec2 ImNodeFlow_Screen2Grid(ImNodeFlowPtr inf, ImVec2 p);
ImVec2 ImNodeFlow_Grid2Screen(ImNodeFlowPtr inf, ImVec2 p);
bool ImNodeFlow_OnSelectedNode(ImNodeFlowPtr inf);
bool ImNodeFlow_OnFreeSpace(ImNodeFlowPtr inf);
void ImNodeFlow_DraggingNode(ImNodeFlowPtr inf, bool state);
void ImNodeFlow_Hovering(ImNodeFlowPtr inf, PinPtr pin);
void ImNodeFlow_HoveredNode(ImNodeFlowPtr inf, BaseNodePtr node);

// BaseNode wrapper functions
BaseNodePtr BaseNode_Create();
void BaseNode_Destroy(BaseNodePtr node);
void BaseNode_Update(BaseNodePtr node);
void BaseNode_SetTitle(BaseNodePtr node, const char* title);
void BaseNode_SetPos(BaseNodePtr node, float x, float y);
void BaseNode_SetHandler(BaseNodePtr node, ImNodeFlowPtr inf);
void BaseNode_SetStyle(BaseNodePtr node, NodeStylePtr style);
void BaseNode_Selected(BaseNodePtr node, bool state);
void BaseNode_UpdatePublicStatus(BaseNodePtr node);
void BaseNode_Destroy_Node(BaseNodePtr node);
bool BaseNode_ToDestroy(BaseNodePtr node);
bool BaseNode_IsHovered(BaseNodePtr node);
bool BaseNode_IsSelected(BaseNodePtr node);
bool BaseNode_IsDragged(BaseNodePtr node);
NodeUID BaseNode_GetUID(BaseNodePtr node);
const char* BaseNode_GetName(BaseNodePtr node);
ImVec2 BaseNode_GetSize(BaseNodePtr node);
ImVec2 BaseNode_GetPos(BaseNodePtr node);
ImNodeFlowPtr BaseNode_GetHandler(BaseNodePtr node);
NodeStylePtr BaseNode_GetStyle(BaseNodePtr node);

// Pin wrapper functions
PinUID Pin_GetUid(PinPtr pin);
const char* Pin_GetName(PinPtr pin);
ImVec2 Pin_GetPos(PinPtr pin);
ImVec2 Pin_GetSize(PinPtr pin);
BaseNodePtr Pin_GetParent(PinPtr pin);
int Pin_GetType(PinPtr pin); // 0 = Input, 1 = Output
PinStylePtr Pin_GetStyle(PinPtr pin);
ImVec2 Pin_PinPoint(PinPtr pin);
float Pin_CalcWidth(PinPtr pin);
void Pin_SetPos(PinPtr pin, float x, float y);
bool Pin_IsConnected(PinPtr pin);
void Pin_CreateLink(PinPtr pin, PinPtr other);
void Pin_DeleteLink(PinPtr pin);

// Link wrapper functions
LinkPtr Link_Create(PinPtr left, PinPtr right, ImNodeFlowPtr inf);
void Link_Destroy(LinkPtr link);
void Link_Update(LinkPtr link);
PinPtr Link_Left(LinkPtr link);
PinPtr Link_Right(LinkPtr link);
bool Link_IsHovered(LinkPtr link);
bool Link_IsSelected(LinkPtr link);

// PinStyle wrapper functions
PinStylePtr PinStyle_Create(ImU32 color, int socket_shape, float socket_radius, 
                           float socket_hovered_radius, float socket_connected_radius, 
                           float socket_thickness);
PinStylePtr PinStyle_Cyan();
PinStylePtr PinStyle_Green();
PinStylePtr PinStyle_Blue();
PinStylePtr PinStyle_Brown();
PinStylePtr PinStyle_Red();
PinStylePtr PinStyle_White();
void PinStyle_Destroy(PinStylePtr style);

// NodeStyle wrapper functions
NodeStylePtr NodeStyle_Create(ImU32 header_bg, ImU32 header_title_color, float radius);
NodeStylePtr NodeStyle_Cyan();
NodeStylePtr NodeStyle_Green();
NodeStylePtr NodeStyle_Red();
NodeStylePtr NodeStyle_Brown();
void NodeStyle_Destroy(NodeStylePtr style);

// Node creation functions
BaseNodePtr ImNodeFlow_AddSimpleNode(ImNodeFlowPtr inf, float x, float y, const char* title);

// Pin creation functions for BaseNode
PinPtr BaseNode_AddInputPin(BaseNodePtr node, const char* name, int data_type);
PinPtr BaseNode_AddOutputPin(BaseNodePtr node, const char* name, int data_type);
PinPtr BaseNode_GetInputPin(BaseNodePtr node, const char* name);
PinPtr BaseNode_GetOutputPin(BaseNodePtr node, const char* name);

// Helper functions
void ImNodeFlow_SmartBezier(ImVec2 p1, ImVec2 p2, ImU32 color, float thickness);
bool ImNodeFlow_SmartBezierCollider(ImVec2 p, ImVec2 p1, ImVec2 p2, float radius);

#ifdef __cplusplus
}
#endif
