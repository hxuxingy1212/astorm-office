#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""按 skill/templates 的每个模板生成"符合体裁与语境、对齐规范"的示例 Word。

用法：python3 scripts/gen_template_samples.py [输出目录]   （默认 /tmp/aiword_templates）

对齐约定：
- 文档标题/摘要标题：居中
- 正文：两端对齐（中文加首行缩进 2 字符 = 24pt）
- 落款/日期：右对齐
- 中文信函：称呼顶格、正文缩进、"此致"空两格、"敬礼！"顶格、落款右对齐
"""
import json, os, sys, glob, subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "release", "json2docx")
OUT = sys.argv[1] if len(sys.argv) > 1 else "/tmp/aiword_templates"


# 由模板与体裁推导的默认（在 main 中设置）
_CFG = {"h1_align": None, "body_align": None, "body_indent": None, "zh": False}


def H1(t):
    return H(1, t, align=_CFG["h1_align"])


def SUB(t):  # 副标题/作者：与标题同对齐（居中标题则居中）
    return P(t, align=(_CFG["h1_align"] or "left"))


def P(t, align=None, indent=None, first_indent=None, bold=None):
    d = {"type": "paragraph", "text": t}
    if align:
        d["align"] = align
    if indent is not None:
        d["indent"] = indent
    if first_indent is not None:
        d["first_line_indent"] = first_indent
    if bold is not None:
        d["bold"] = bold
    return d


def H(level, t, align=None):
    d = {"type": "heading", "level": level, "text": t}
    if align:
        d["align"] = align
    return d


def BODY(t):          # 正文：遵循模板 Normal 对齐/缩进，缺省中文两端对齐+缩进
    a = _CFG["body_align"] or ("justify" if _CFG["zh"] else None)
    ind = _CFG["body_indent"] if _CFG["body_indent"] is not None else (24 if _CFG["zh"] else None)
    return P(t, align=a, first_indent=ind)


def ENBODY(t):        # 英文正文：遵循模板 Normal
    a = _CFG["body_align"]
    ind = _CFG["body_indent"]
    return P(t, align=a, first_indent=ind)


def L(items, ordered=False):
    return {"type": "list", "ordered": ordered,
            "items": [{"blocks": [P(x)]} for x in items]}


def TBL(rows, caption=None, header=True):
    return {"type": "table", "header_row": header, "caption": caption,
            "rows": [{"cells": [{"blocks": [P(c)]} for c in r]} for r in rows]}


def SUB(t):
    return P(t, align="center")


def RIGHT(t):
    return P(t, align="right")


# ---------------- 各领域 ----------------

def zh_paper():
    return [
        H1("基于深度学习的多模态医学影像分割方法研究"),
        SUB("A Deep Learning Approach for Multimodal Medical Image Segmentation"),
        SUB("张三  李四  王五　（某某大学 计算机科学与技术学院，北京 100084）"),
        H(2, "摘要", align="center"),
        P("针对多模态医学影像中模态异质性与标注稀缺问题，本文提出了一种跨模态特征对齐与融合的分割网络。"
          "该方法通过共享编码器提取模态共性特征，并引入注意力引导的跨模态对齐模块，实现信息互补；"
          "在公开数据集上的实验表明，本文方法在 Dice 系数与 HD95 指标上均优于基线模型。", align="justify"),
        P("关键词：医学影像分割；多模态融合；注意力机制；深度学习"),
        H(2, "1 引言"),
        BODY("医学影像分割是计算机辅助诊断的关键步骤，其目标是从 CT、MRI、超声等影像中精确勾画出器官与病灶的边界。"
             "传统方法依赖人工设计的特征，难以适应模态间的巨大差异。近年来，深度学习显著提升了分割精度，"
             "但多模态数据中的配准误差、模态缺失与标注成本仍是亟待解决的问题。"),
        H(3, "1.1 研究背景"),
        BODY("多模态影像能够提供互补的解剖与功能信息，是提高分割鲁棒性的重要途径。"),
        H(2, "2 相关工作"),
        BODY("现有融合策略可分为早期融合、晚期融合与混合融合。早期融合在输入层拼接模态，简单但对配准敏感；"
             "晚期融合在各模态独立预测后集成，忽略了模态间的互补性。"),
        H(2, "3 方法"),
        BODY("给定模态集合 X = {x1, x2, …, xM}，网络首先由共享编码器得到特征，再经对齐模块计算跨模态注意力："),
        {"type": "formula", "latex": "A = \\mathrm{softmax}(\\frac{Q K^{T}}{\\sqrt{d}}) V",
         "display": True, "number": "(1)"},
        BODY("其中 Q、K、V 分别为查询、键与值矩阵，d 为特征维度。融合后的特征送入解码器输出分割掩膜。"),
        H(2, "4 实验"),
        BODY("实验在 BraTS、ISIC 与自建数据集上进行，采用 Adam 优化器，初始学习率 1e-4。"),
        TBL([["方法", "Dice (%)", "HD95 (mm)", "参数量 (M)"],
             ["U-Net", "82.1", "9.4", "31.0"],
             ["Attention U-Net", "84.6", "7.8", "34.9"],
             ["本文方法", "88.3", "5.2", "26.4"]],
            caption="表 1  不同方法在 BraTS 数据集上的分割性能对比"),
        BODY("由表 1 可见，本文方法在 Dice 与 HD95 上均取得最优结果，且参数量更小，验证了跨模态对齐的有效性。"),
        H(2, "5 结论"),
        BODY("本文提出了一种跨模态特征对齐与融合的分割网络，在多个数据集上取得了领先性能。"
             "未来工作将进一步研究模态缺失场景下的鲁棒性与模型的可解释性。"),
        H(2, "参考文献"),
        {"type": "bibliography", "style": "GB/T 7714", "entries": [
            {"kind": "article", "authors": ["Ronneberger O", "Fischer P", "Brox T"],
             "title": "U-Net: Convolutional networks for biomedical image segmentation",
             "container": "MICCAI", "year": 2015},
            {"kind": "article", "authors": ["Oktay O", "Schlemper J"],
             "title": "Attention U-Net: Learning where to look for the pancreas",
             "container": "MIDL", "year": 2018},
        ]},
    ]


def en_paper():
    return [
        H1("Attention-Based Anomaly Detection for Industrial Time Series"),
        SUB("A Case Study on Rotating Machinery"),
        SUB("Alice Chen, Bob Martinez — Department of Computer Science, Stanford University"),
        H(2, "Abstract", align="center"),
        P("We present an attention-based model for detecting anomalies in industrial sensor time series. "
          "The model combines convolutional feature extraction with a temporal attention mechanism, "
          "and achieves state-of-the-art F1 scores on three benchmark datasets while remaining lightweight enough for edge deployment."),
        P("Keywords: anomaly detection; time series; attention; predictive maintenance"),
        H(2, "1. Introduction"),
        ENBODY("Industrial equipment generates vast amounts of multivariate sensor data. Early detection of anomalies "
               "is critical to predictive maintenance, yet labeled faults are rare and highly imbalanced."),
        H(2, "2. Method"),
        ENBODY("Given a window of sensor readings, a 1D convolutional encoder produces latent features, "
               "which are weighted by a learned temporal attention. Reconstruction error is used as the anomaly score."),
        H(2, "3. Experiments"),
        TBL([["Model", "Precision", "Recall", "F1"],
             ["Isolation Forest", "0.71", "0.64", "0.67"],
             ["LSTM-AE", "0.80", "0.75", "0.77"],
             ["Ours", "0.91", "0.88", "0.89"]],
            caption="Table 1. Detection performance on the SMD benchmark"),
        ENBODY("Our method outperforms all baselines by a clear margin, with an average F1 improvement of 0.12."),
        H(2, "4. Conclusion"),
        ENBODY("We introduced a lightweight attention model for industrial anomaly detection. "
               "Future work will explore multimodal signals and online adaptation."),
        H(2, "References"),
        {"type": "bibliography", "style": "APA", "entries": [
            {"kind": "article", "authors": ["Su, Y."], "title": "Robust anomaly detection for multivariate time series",
             "container": "VLDB", "year": 2019},
            {"kind": "article", "authors": ["Vaswani, A."], "title": "Attention is all you need",
             "container": "NeurIPS", "year": 2017},
        ]},
    ]


def zh_report():
    return [
        H1("2026 年第三季度经营分析报告"),
        SUB("业务增长、渠道结构与风险提示"),
        SUB("战略规划部　2026 年 10 月"),
        H(2, "一、总体概况"),
        BODY("本季度公司实现营业收入 12.8 亿元，同比增长 18.4%，环比增长 6.1%；"
             "毛利率 41.2%，较上季度提升 1.3 个百分点，主要得益于高毛利产品占比上升与供应链降本。"),
        H(2, "二、分渠道表现"),
        TBL([["渠道", "收入（亿元）", "同比", "占比"],
             ["直营", "6.2", "+22%", "48%"],
             ["经销", "4.1", "+12%", "32%"],
             ["线上", "2.5", "+31%", "20%"]],
            caption="表 1  各渠道收入与增长"),
        BODY("线上渠道增速领先，但获客成本上升；经销渠道库存周转天数由 58 天降至 51 天，渠道健康度改善。"),
        H(2, "三、重点工作与进展"),
        L(["完成新一代产品的量产爬坡，良率提升至 96.5%；",
           "华东与华南两大区域完成组织架构调整；",
           "上线统一数据看板，经营指标实现日报化。"], ordered=True),
        H(2, "四、风险提示与建议"),
        BODY("原材料价格波动与海外需求不确定性仍是主要风险。建议加强核心物料的长协锁定，"
             "稳步推进海外本地化布局，并持续优化产品结构以提升盈利质量。"),
        RIGHT("编制：战略规划部　2026 年 10 月 15 日"),
    ]


def zh_official():
    return [
        H1("关于开展 2026 年度安全生产大检查的通知"),
        P("各分公司、各部门："),
        BODY("为深入贯彻落实上级关于安全生产的部署要求，切实防范和化解各类安全风险，"
             "经研究，决定于 2026 年 10 月至 12 月在全公司范围内开展安全生产大检查。现将有关事项通知如下："),
        H(2, "一、检查范围"),
        BODY("各生产基地、仓储中心、在建工程项目及办公场所，重点检查消防、用电、特种设备与危险化学品管理。"),
        H(2, "二、检查内容"),
        L(["安全生产责任制落实情况；",
           "隐患排查治理台账建立与闭环情况；",
           "应急预案演练与培训记录；",
           "特种作业人员持证上岗情况。"], ordered=True),
        H(2, "三、工作安排"),
        BODY("本次检查分为自查自纠（10 月）、集中检查（11 月）与整改提升（12 月）三个阶段，"
             "各单位须于每月末前报送阶段性进展。"),
        H(2, "四、工作要求"),
        BODY("各单位主要负责人要亲自部署、亲自督办，对检查中发现的重大隐患实行挂牌督办，"
             "确保整改责任、措施、资金、时限、预案“五落实”。"),
        P("特此通知。"),
        RIGHT("某某集团有限公司"),
        RIGHT("2026 年 9 月 29 日"),
    ]


def zh_letter():
    return [
        P("尊敬的客户："),
        BODY("您好！感谢您长期以来对我公司的信任与支持。"),
        BODY("现就贵单位所咨询的合作事宜，答复如下：我司将自 2026 年 10 月 1 日起，"
             "对现有服务方案进行全面升级，新增 7×24 小时技术支持与季度巡检服务，服务费用保持不变。"),
        BODY("相关协议文本将随后通过邮件发送，敬请查收。如对条款有任何疑问，欢迎随时与我司客户经理联系。"),
        P("此致", first_indent=24),
        P("敬礼！"),
        RIGHT("某某科技有限公司"),
        RIGHT("2026 年 9 月 29 日"),
    ]


def en_letter():
    return [
        RIGHT("29 September 2026"),
        P(""),
        P("Dear Ms. Johnson,"),
        ENBODY("Thank you for your continued partnership with our firm. I am writing to confirm the terms "
               "discussed during our meeting on 20 September 2026 regarding the renewal of the service agreement."),
        ENBODY("As agreed, the renewed agreement will take effect on 1 January 2027 for a term of 24 months, "
               "with the same service scope and a fixed annual fee. A detailed schedule is enclosed for your review."),
        ENBODY("Please do not hesitate to contact me should you have any questions. "
               "We look forward to another successful year of collaboration."),
        P("Yours sincerely,"),
        P("Alex Martin"),
        P("Director of Client Services"),
    ]


def ja_doc():
    return [
        H1("深層学習を用いた文書要約に関する研究"),
        SUB("Document Summarization with Deep Learning"),
        H(2, "概要", align="center"),
        ENBODY("本研究では、大規模言語モデルを用いた日本語文書の自動要約手法を提案する。"
               "提案手法は、文単位の重要度推定と冗長性除去を組み合わせ、要約の一貫性と簡潔性を両立する。"),
        H(2, "1. はじめに"),
        ENBODY("文書量の増大に伴い、効率的な情報把握のための自動要約の重要性が高まっている。"),
        H(2, "2. 提案手法"),
        ENBODY("入力文書を文に分割し、各文の重要度スコアを推定した後、冗長な文を除去して要約を生成する。"),
        H(2, "3. 実験"),
        TBL([["手法", "ROUGE-1", "ROUGE-2", "ROUGE-L"],
             ["Lead-3", "0.38", "0.15", "0.34"],
             ["Seq2Seq", "0.41", "0.18", "0.37"],
             ["提案手法", "0.46", "0.21", "0.42"]],
            caption="表1 各手法の要約性能"),
        ENBODY("提案手法は全ての指標で既存手法を上回り、特に ROUGE-1 で 5 ポイントの改善が得られた。"),
        H(2, "おわりに"),
        ENBODY("本稿では文書要約のためのハイブリッド手法を提案し、その有効性を確認した。"),
    ]


def design_doc():
    return [
        H1("品牌视觉规范纲要"),
        SUB("Brand Visual Guidelines — v1.0"),
        SUB("设计中心"),
        H(2, "1. 设计原则"),
        ENBODY("本规范旨在统一品牌在数字与印刷媒介上的表达。核心原则为：简洁、聚焦、留白充足、层级清晰。"),
        H(2, "2. 色彩系统"),
        TBL([["角色", "色值", "用途"],
             ["主色", "#5B9BD5", "标题与强调"],
             ["辅色", "#ED7D31", "行动号召"],
             ["中性色", "#333333", "正文"],
             ["背景", "#F7F9FB", "区块底色"]],
            caption="表 1  品牌色彩系统"),
        H(2, "3. 字体与排版"),
        ENBODY("中英文均以无衬线字体为主，标题字重加粗、字距收紧；正文行距 1.6 倍，保证可读性。"),
        H(2, "4. 版式与应用"),
        L(["封面：大标题居左，留白不少于 40%；",
           "内页：统一的页眉页脚与网格系统；",
           "图表：限用品牌色系，避免高饱和撞色。"]),
        ENBODY("以上规范适用于产品界面、宣传物料与对外演示文档，后续如有更新以最新版本为准。"),
    ]


def tech_doc():
    return [
        H1("用户服务 API 接口文档"),
        SUB("User Service API Reference v2.3"),
        H(2, "1. 概述"),
        ENBODY("本文档描述用户服务的对外 REST API，包括鉴权方式、接口清单、请求/响应格式与错误码约定。"
               "所有接口均以 JSON 传输，基地址为 https://api.example.com/v2。"),
        H(2, "2. 鉴权"),
        ENBODY("除公开接口外，均需在请求头携带 Bearer Token："),
        {"type": "code", "lang": "http", "text": "Authorization: Bearer <access_token>"},
        H(2, "3. 接口清单"),
        TBL([["方法", "路径", "说明"],
             ["GET", "/users/{id}", "查询用户详情"],
             ["POST", "/users", "创建用户"],
             ["PUT", "/users/{id}", "更新用户"],
             ["DELETE", "/users/{id}", "删除用户"]],
            caption="表 1  接口清单"),
        H(2, "4. 示例"),
        ENBODY("创建用户的请求示例："),
        {"type": "code", "lang": "json", "text": "POST /v2/users\n{\n  \"name\": \"张三\",\n  \"email\": \"zhangsan@example.com\"\n}"},
        ENBODY("成功响应（201 Created）："),
        {"type": "code", "lang": "json", "text": "{\n  \"id\": \"u_1001\",\n  \"name\": \"张三\",\n  \"created_at\": \"2026-09-29T10:00:00Z\"\n}"},
        H(2, "5. 错误码"),
        TBL([["HTTP", "code", "说明"],
             ["400", "INVALID_ARGUMENT", "参数校验失败"],
             ["401", "UNAUTHENTICATED", "未认证或令牌过期"],
             ["404", "NOT_FOUND", "资源不存在"],
             ["429", "RATE_LIMITED", "请求过于频繁"]],
            caption="表 2  错误码"),
        ENBODY("出现错误时，响应体包含 code 与 message 字段，便于调用方定位问题。"),
    ]


def corporate_doc():
    return [
        H1("数据安全管理制度（试行）"),
        H(2, "第一章 总则"),
        BODY("第一条 为规范公司数据资产管理，保障数据安全与合规使用，依据国家相关法律法规，制定本制度。"),
        BODY("第二条 本制度适用于公司全体员工及合作方在数据处理活动中的行为。"),
        H(2, "第二章 职责分工"),
        L(["信息安全委员会负责制度审定与重大事项决策；",
           "信息安全部负责日常监督、检查与应急响应；",
           "各业务部门负责本部门数据的分类分级与权限管理。"], ordered=True),
        H(2, "第三章 数据分类分级"),
        BODY("数据按敏感程度分为公开、内部、机密、绝密四级，不同级别对应差异化的存储、传输与访问控制要求。"),
        H(2, "第四章 安全措施"),
        BODY("包括但不限于：最小权限原则、操作留痕与审计、加密存储与传输、定期备份与演练。"),
        H(2, "第五章 附则"),
        BODY("本制度自发布之日起施行，由信息安全部负责解释与修订。"),
        RIGHT("某某科技有限公司"),
        RIGHT("2026 年 9 月 29 日"),
    ]


def en_report():
    return [
        H1("Strategic Growth Review — FY26 Outlook"),
        SUB("Prepared for the Leadership Team"),
        SUB("Strategy & Operations"),
        H(2, "1. Executive Summary"),
        ENBODY("The business delivered resilient growth in FY25 despite macro headwinds. This review outlines "
               "three strategic priorities for FY26: margin resilience, EMEA expansion, and disciplined capital allocation."),
        H(2, "2. Market Context"),
        ENBODY("Demand remains solid in core segments, while competitive intensity has increased in the mid-market."),
        H(2, "3. Financial Snapshot"),
        TBL([["Metric", "FY24", "FY25", "YoY"],
             ["Revenue ($M)", "412", "486", "+18%"],
             ["Gross margin", "39.1%", "41.2%", "+2.1pt"],
             ["EBITDA ($M)", "74", "92", "+24%"]],
            caption="Table 1. Key financials"),
        H(2, "4. Strategic Priorities"),
        L(["Protect margin through supply-chain optimization;",
           "Establish EMEA beachhead with two flagship customers;",
           "Rebalance R&D toward platform reuse."], ordered=True),
        H(2, "5. Recommendations"),
        ENBODY("We recommend approving the FY26 plan with a staged investment gate at the end of Q2, "
               "subject to EMEA pipeline conversion and gross-margin targets."),
    ]



def resume_doc():
    return [
        H1("张三"),
        SUB("后端工程师　|　138-0000-0000　|　zhangsan@example.com"),
        H(2, "教育背景"),
        P("某某大学 计算机科学与技术 硕士　2019.09–2022.06"),
        H(2, "工作经历"),
        P("某某科技 高级后端工程师　2022.07–至今"),
        L(["主导订单系统的服务拆分与性能优化，QPS 提升 3 倍；",
           "搭建统一网关与鉴权体系，支撑 20+ 业务方接入。"]),
        H(2, "项目经验"),
        P("分布式任务调度平台：基于一致性哈希与租约机制实现高可用调度。"),
        H(2, "技能"),
        L(["语言：Go / Java / Rust", "中间件：Kafka / Redis / MySQL", "云原生：Docker / Kubernetes"]),
    ]


def contract_doc():
    return [
        H1("技术服务合同"),
        P("甲方：某某集团有限公司"),
        P("乙方：某某科技有限公司"),
        BODY("甲乙双方本着平等自愿、诚实信用的原则，就技术服务事宜达成如下协议，共同遵守。"),
        H(2, "第一条 服务内容"),
        BODY("乙方向甲方提供信息系统运维与技术支持服务，具体范围以附件《服务清单》为准。"),
        H(2, "第二条 服务期限"),
        BODY("本合同服务期为 12 个月，自 2026 年 10 月 1 日起至 2027 年 9 月 30 日止。"),
        H(2, "第三条 服务费用与支付"),
        BODY("服务费为人民币壹拾贰万元整，分四期按季度支付。甲方应于每季度首月 10 日前支付当期费用。"),
        H(2, "第四条 保密条款"),
        BODY("双方应对履约过程中知悉的对方商业秘密予以保密，未经书面同意不得向第三方披露。"),
        H(2, "第五条 争议解决"),
        BODY("因本合同产生的争议，双方应友好协商解决；协商不成的，提交甲方所在地人民法院诉讼解决。"),
        P("（以下无正文）"),
        RIGHT("甲方（盖章）：某某集团有限公司"),
        RIGHT("乙方（盖章）：某某科技有限公司"),
        RIGHT("签订日期：2026 年 9 月 29 日"),
    ]


def minutes_doc():
    return [
        H1("某某项目周例会会议纪要"),
        P("时间：2026 年 9 月 29 日 14:00–15:30　　地点：三号会议室"),
        P("参会：张三、李四、王五　　记录：王五"),
        H(2, "一、本周进展"),
        BODY("完成核心模块开发与联调，接口联调通过率 95%；测试环境部署完成。"),
        H(2, "二、问题与决议"),
        L(["性能瓶颈：决定引入缓存层，由李四负责；",
           "接口变更：统一走网关，下周完成改造。"], ordered=True),
        H(2, "三、待办事项"),
        TBL([["事项", "负责人", "期限", "状态"],
             ["缓存层设计与实现", "李四", "10-07", "进行中"],
             ["网关改造", "张三", "10-10", "未开始"],
             ["测试用例补充", "王五", "10-09", "进行中"]],
            caption="表 1  待办事项"),
    ]


def generic_zh():
    return [
        H1("项目实施方案"),
        SUB("背景、目标与实施路径"),
        SUB("项目组"),
        H(2, "一、项目背景"),
        BODY("为提升业务效率与数据质量，拟建设统一的信息化平台，整合分散的业务系统，形成端到端的数据闭环。"),
        H(2, "二、建设目标"),
        L(["统一数据标准与主数据管理；",
           "打通核心业务流程，减少人工干预；",
           "建立可视化经营分析能力。"], ordered=True),
        H(2, "三、实施计划"),
        TBL([["阶段", "时间", "主要任务"],
             ["需求调研", "第 1 月", "现状梳理与需求确认"],
             ["系统设计", "第 2 月", "架构与接口设计"],
             ["开发测试", "第 3–5 月", "迭代开发与联调"],
             ["上线运维", "第 6 月", "试运行与优化"]],
            caption="表 1  实施计划"),
        H(2, "四、保障措施"),
        BODY("成立项目领导小组与专项工作组，明确职责分工；建立周例会与里程碑评审机制，确保项目按期高质量交付。"),
    ]


def generic_en():
    return [
        H1("Project Plan and Status Update"),
        SUB("Scope, milestones and next steps"),
        SUB("Program Office"),
        H(2, "1. Background"),
        ENBODY("This initiative consolidates fragmented tools into a single platform, enabling end-to-end data flow and better decision making."),
        H(2, "2. Objectives"),
        L(["Establish unified data standards;", "Automate core workflows;", "Deliver executive analytics."], ordered=True),
        H(2, "3. Milestones"),
        TBL([["Phase", "Timeline", "Deliverable"],
             ["Discovery", "Month 1", "Requirements"],
             ["Design", "Month 2", "Architecture"],
             ["Build", "Months 3–5", "System & tests"],
             ["Launch", "Month 6", "Go-live"]],
            caption="Table 1. Milestones"),
        H(2, "4. Next Steps"),
        ENBODY("The steering committee will review progress at each milestone gate, with a go/no-go decision before launch."),
    ]


BUILDERS = {
    "chinese-sci-numbered": zh_paper, "chinese-sci": zh_paper,
    "academic-cambria": en_paper, "academic-arial": en_paper, "classic-times": en_paper,
    "chinese-modern": zh_report, "chinese-official": zh_official, "chinese-letter": zh_letter,
    "japanese-serif": ja_doc, "design-helvetica": design_doc, "report-cambria": en_report,
    "corporate-arial": corporate_doc, "technical-arial": tech_doc,
    "modern-calibri": generic_en, "modern-aptos": generic_en, "french-letter": en_letter,
    "classic-palatino": en_report, "sans-trebuchet": generic_en,
    "resume": resume_doc, "contract": contract_doc, "meeting-minutes": minutes_doc,
}


def feature_showcase():
    """展示工具新特性的样张：盒式边框/制表位前导符/富图表/SDT/表单域/域/注音/RTL/表格宽度。"""
    return [
        H(1, "新特性样张", align="center"),
        P("段落盒式边框", align="center"),
        {"type": "paragraph", "text": "带四边盒式边框的段落（border_box）。",
         "border_box": "4F81BD", "shading": "F3F3F3"},
        {"type": "paragraph", "runs": [
            {"text": "目录项"},
            {"text": "	1", "tabs": None},
        ]},
        {"type": "paragraph", "text": "制表位点线前导", "tabs": [{"pos": 300, "align": "right", "leader": "dot"}]},
        {"type": "paragraph", "text": "制表位虚线前导", "tabs": [{"pos": 300, "align": "right", "leader": "hyphen"}]},
        {"type": "chart", "chart_type": "column", "title": "季度营收", "grouping": "stacked",
         "legend": "r", "x_title": "季度", "y_title": "万元", "y_min": 0, "y_max": 400,
         "data_labels": True, "categories": ["Q1", "Q2", "Q3"],
         "series": [{"name": "营收", "values": [120, 150, 180], "color": "4472C4"},
                    {"name": "成本", "values": [80, 90, 100], "color": "ED7D31"}]},
        {"type": "table", "header_row": True, "width": 400, "layout": "fixed",
         "style": "LightShading", "caption": "表 1  新特性",
         "rows": [{"height": 24, "height_rule": "atLeast", "cells": [
             {"blocks": [P("项")], "fill": "F2F2F2", "valign": "center", "margin": 4.0},
             {"blocks": [P("值")], "fill": "F2F2F2", "valign": "center"}]},
             {"cells": [{"blocks": [P("宽度/布局")]}, {"blocks": [P("width=400, layout=fixed")]}]}]},
        {"type": "paragraph", "runs": [
            {"text": "域：", "bold": True},
            {"text": "第 X 页", "field": "PAGE"},
            {"text": "（表单：", },
            {"text": "文本值", "form_field": {"kind": "text", "name": "Text1"}},
            {"text": "）"}]},
        {"type": "paragraph", "runs": [
            {"text": "符号 ", },
            {"text": "", "symbol": "Wingdings:F0B7"},
            {"text": "  注音 "},
            {"text": "東京", "ruby": "とうきょう"},
            {"text": "  RTL ", "rtl": True, "lang": "ar-SA"}]},
        {"type": "sdt", "sdt_type": "dropDownList", "alias": "状态", "tag": "status",
         "items": ["进行中", "已完成"],
         "blocks": [P("进行中")]},
    ]


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sorted(os.path.basename(os.path.dirname(p))
                   for p in glob.glob(os.path.join(ROOT, "skill/templates/*/template.json")))
    for name in names:
        tpl = json.load(open(os.path.join(ROOT, "skill/templates", name, "template.json")))
        builder = BUILDERS.get(name, generic_zh)
        st = tpl.get("styles") or {}
        h1 = (st.get("Heading1") or {}).get("align")
        nrm = st.get("Normal") or {}
        genre = ("official" if "official" in name else
                 "japanese" if "japanese" in name else
                 "design" if "design" in name else
                 "letter" if "letter" in name else
                 "tech" if "technical" in name else
                 "paper" if ("sci" in name or "academic" in name) else
                 "report")
        default_h1 = {"official": "center", "japanese": "center", "report": "center",
                      "tech": "center", "design": "left", "paper": "left", "letter": None}.get(genre)
        _CFG["h1_align"] = h1 if h1 is not None else default_h1
        _CFG["body_align"] = nrm.get("align")
        _CFG["body_indent"] = nrm.get("first_line_indent")
        _CFG["zh"] = ("chinese" in name) or ("japanese" in name)
        doc = {"page": tpl.get("page"), "theme": tpl.get("theme"), "styles": tpl.get("styles"),
               "parts": [{"blocks": builder()}]}
        d = os.path.join("/tmp/_tplbuild", name)
        os.makedirs(d, exist_ok=True)
        json.dump(doc, open(os.path.join(d, "document.json"), "w"), ensure_ascii=False, indent=2)
        out = os.path.join(OUT, f"{name}.docx")
        r = subprocess.run([BIN, "repack", d, "-o", out], capture_output=True, text=True)
        print(f"{name:24} {'ok' if r.returncode == 0 else 'FAIL ' + r.stderr[:80]}")

    # 新特性样张
    d = os.path.join("/tmp/_tplbuild", "feature-showcase")
    os.makedirs(d, exist_ok=True)
    doc = {"page": {"size": "A4"}, "parts": [{"blocks": feature_showcase()}]}
    json.dump(doc, open(os.path.join(d, "document.json"), "w"), ensure_ascii=False, indent=2)
    out = os.path.join(OUT, "feature-showcase.docx")
    r = subprocess.run([BIN, "repack", d, "-o", out], capture_output=True, text=True)
    print(f"{'feature-showcase':24} {'ok' if r.returncode == 0 else 'FAIL ' + r.stderr[:120]}")


if __name__ == "__main__":
    main()
