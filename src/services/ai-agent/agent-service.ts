import type {
  ActionableFix,
  ChatMessage,
  DiagnosticStep,
  DiagnosticVisualData,
  VertexAgentConfig,
} from "../../types/agent";
import {
  ADK_DIAGNOSTIC_TOOLS,
  executeDiagnosticTool,
  type ToolExecutionResponse,
} from "./tools";

export interface AgentExecutionCallbacks {
  onStepAdded?: (step: DiagnosticStep) => void;
  onStepUpdated?: (step: DiagnosticStep) => void;
  onTextChunk?: (chunk: string) => void;
  onFixesIdentified?: (fixes: ActionableFix[]) => void;
}

const DEFAULT_CONFIG: VertexAgentConfig = {
  projectId: "",
  location: "global",
  model: "gemini-3.5-flash-lite",
  apiKey: "",
  useLocalFallbackIfUnconfigured: true,
};

const SYSTEM_INSTRUCTION = `You are ASAO AI, an intelligent, conversational system-diagnostics assistant running directly inside the ASAO PC optimization app on Windows.
Your role is to diagnose system performance issues, answer natural language questions about the user's PC, and provide deep, multi-step technical explanations, actionable recommendations, and fixes.

CAPABILITIES & TOOLS:
You have access to real-time system diagnostic tools:
1. Windows Startup Applications and boot-time delay (check_startup_applications)
2. Running processes, background strain, CPU & RAM consumption (check_background_processes)
3. Disk storage capacity, categories breakdown, large files & folders (check_storage_status)
4. Storage cleanup opportunities, cache, junk, and temp files (check_storage_insights)
5. Overall system health metrics and strain (check_system_health_overview)

CRITICAL INSTRUCTIONS:
- TARGETED TOOL SELECTION: ONLY invoke tools that are directly relevant to the user's specific inquiry.
  * If the user asks specifically about startup applications or boot-time delay, invoke ONLY check_startup_applications. Do NOT invoke storage or process tools unless requested.
  * If the user asks about memory, RAM usage, or CPU-heavy processes, invoke check_background_processes and check_system_health_overview. Do NOT invoke startup or storage tools.
  * If the user asks about disk space or storage capacity, invoke check_storage_status.
  * If the user asks a broad question like "Why is my PC slow?" or "Run a full system check", perform a multi-point diagnosis by invoking multiple relevant tools.
  * If the user asks a general or conceptual question (e.g. "What is a good boot time?", "How does virtual memory work?"), do NOT call diagnostic tools; provide a thoughtful, educational response directly.

- RESPONSE STYLE - MULTI-STEP & IN-DEPTH AI RESPONSE:
  Write a thorough, intelligent, and well-structured response using markdown:
  1. Executive Diagnosis: Clear, direct answer summarizing what was found and why it affects the PC.
  2. Technical Breakdown: Deep-dive into specific culprits (process names, startup apps, delay ms, memory footprint).
  3. Multi-Step Remediation Plan: Provide numbered steps (1., 2., 3.) outlining concrete optimization actions (quick immediate fixes, deeper configuration settings, and preventive habits).
  4. ASAO 1-Click Fixes: Mention the specific actions available right in the UI to resolve the issue.

- Be precise, polite, authoritative, and helpful. Do NOT mention internal tool schemas or raw JSON.`;

export class VertexAdkAgentService {
  private config: VertexAgentConfig;

  constructor() {
    this.config = this.loadConfig();
  }

  public getConfig(): VertexAgentConfig {
    return { ...this.config };
  }

  public updateConfig(newConfig: Partial<VertexAgentConfig>): void {
    this.config = { ...this.config, ...newConfig };
    this.saveConfig(this.config);
  }

  private loadConfig(): VertexAgentConfig {
    try {
      const stored = localStorage.getItem("asao_vertex_agent_config");
      if (stored) {
        return { ...DEFAULT_CONFIG, ...JSON.parse(stored) };
      }
    } catch {
      // Fallback
    }
    return { ...DEFAULT_CONFIG };
  }

  private saveConfig(config: VertexAgentConfig): void {
    try {
      localStorage.setItem("asao_vertex_agent_config", JSON.stringify(config));
    } catch {
      // Fallback
    }
  }

  /**
   * Main entry point to ask the AI Diagnostics Assistant.
   */
  public async processQuery(
    userPrompt: string,
    history: ChatMessage[],
    callbacks?: AgentExecutionCallbacks
  ): Promise<{
    text: string;
    steps: DiagnosticStep[];
    fixes: ActionableFix[];
    diagnosticData?: DiagnosticVisualData;
  }> {
    const steps: DiagnosticStep[] = [];

    const addStep = (label: string, toolName?: string): DiagnosticStep => {
      const step: DiagnosticStep = {
        id: `step-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        label,
        toolName,
        status: "running",
        startedAt: Date.now(),
      };
      steps.push(step);
      callbacks?.onStepAdded?.(step);
      return step;
    };

    const updateStep = (
      step: DiagnosticStep,
      status: "completed" | "error",
      detail?: string
    ) => {
      step.status = status;
      step.detail = detail;
      step.durationMs = Date.now() - (step.startedAt ?? Date.now());
      callbacks?.onStepUpdated?.(step);
    };

    const hasLiveVertexCredentials =
      Boolean(this.config.projectId?.trim()) &&
      (Boolean(this.config.apiKey?.trim()) || Boolean(this.config.authToken?.trim()));

    if (hasLiveVertexCredentials) {
      try {
        const liveResult = await this.executeLiveVertexLoop(
          userPrompt,
          history,
          steps,
          addStep,
          updateStep,
          callbacks
        );
        return liveResult;
      } catch (err) {
        console.warn("Live Vertex AI query encountered an issue, falling back to ASAO native diagnostic engine:", err);
      }
    }

    // ASAO Native Diagnostic Engine
    const fallbackResult = await this.executeNativeDiagnosticEngine(
      userPrompt,
      steps,
      addStep,
      updateStep,
      callbacks
    );

    return fallbackResult;
  }

  /**
   * Executes multi-turn tool calling with Google Vertex AI API (Gemini 3.5 Flash-Lite).
   */
  private async executeLiveVertexLoop(
    userPrompt: string,
    history: ChatMessage[],
    steps: DiagnosticStep[],
    addStep: (label: string, toolName?: string) => DiagnosticStep,
    updateStep: (step: DiagnosticStep, status: "completed" | "error", detail?: string) => void,
    callbacks?: AgentExecutionCallbacks
  ): Promise<{
    text: string;
    steps: DiagnosticStep[];
    fixes: ActionableFix[];
    diagnosticData?: DiagnosticVisualData;
  }> {
    const initStep = addStep("Analyzing query & selecting diagnostics tools...");
    await new Promise((r) => setTimeout(r, 120));
    updateStep(initStep, "completed", "Tools selected based on query intent");

    const toolsDecl = [
      {
        functionDeclarations: ADK_DIAGNOSTIC_TOOLS.map((t) => ({
          name: t.name,
          description: t.description,
          parameters: t.parameters,
        })),
      },
    ];

    // Build contents history
    const contents: Array<{
      role: "user" | "model" | "function";
      parts: Array<Record<string, unknown>>;
    }> = [];

    // Add recent history (up to last 4 messages)
    const recentHistory = history.slice(-4);
    for (const msg of recentHistory) {
      contents.push({
        role: msg.sender === "user" ? "user" : "model",
        parts: [{ text: msg.text }],
      });
    }

    contents.push({
      role: "user",
      parts: [{ text: userPrompt }],
    });

    const projectId = this.config.projectId.trim();
    const location = (this.config.location || "global").trim();
    const model = (this.config.model || "gemini-3.5-flash-lite").trim();

    let endpoint = `https://aiplatform.googleapis.com/v1/projects/${projectId}/locations/${location}/publishers/google/models/${model}:generateContent`;

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };

    if (this.config.authToken?.trim()) {
      headers["Authorization"] = `Bearer ${this.config.authToken.trim()}`;
    } else if (this.config.apiKey?.trim()) {
      headers["x-goog-api-key"] = this.config.apiKey.trim();
      endpoint += `?key=${this.config.apiKey.trim()}`;
    }

    let iterations = 0;
    const maxIterations = 5;
    const allCollectedToolResponses: ToolExecutionResponse[] = [];

    while (iterations < maxIterations) {
      iterations++;

      const requestPayload = {
        contents,
        systemInstruction: {
          parts: [{ text: SYSTEM_INSTRUCTION }],
        },
        tools: toolsDecl,
      };

      const res = await fetch(endpoint, {
        method: "POST",
        headers,
        body: JSON.stringify(requestPayload),
      });

      if (!res.ok) {
        const errorText = await res.text().catch(() => "Unknown API error");
        throw new Error(`Vertex AI API error (${res.status}): ${errorText}`);
      }

      const responseJson = await res.json();
      const candidate = responseJson?.candidates?.[0];
      const modelParts = candidate?.content?.parts || [];

      // Check if model returned function calls
      const functionCalls = modelParts.filter(
        (p: Record<string, unknown>) => p.functionCall
      );

      if (functionCalls.length > 0) {
        // Model requested tool calls
        contents.push({
          role: "model",
          parts: modelParts,
        });

        const functionResponseParts: Array<Record<string, unknown>> = [];

        for (const call of functionCalls) {
          const fn = call.functionCall as { name: string; args: Record<string, unknown> };
          const toolStep = addStep(this.getToolStepDisplayLabel(fn.name), fn.name);

          try {
            const toolResult = await executeDiagnosticTool(fn.name, fn.args || {});
            allCollectedToolResponses.push(toolResult);
            updateStep(toolStep, "completed", toolResult.summary);

            functionResponseParts.push({
              functionResponse: {
                name: fn.name,
                response: {
                  content: toolResult.data,
                },
              },
            });
          } catch (err) {
            updateStep(toolStep, "error", err instanceof Error ? err.message : String(err));
            functionResponseParts.push({
              functionResponse: {
                name: fn.name,
                response: {
                  error: err instanceof Error ? err.message : "Tool execution failed",
                },
              },
            });
          }
        }

        contents.push({
          role: "function",
          parts: functionResponseParts,
        });

        // Continue loop to allow the model to review responses or call more tools
        continue;
      }

      // No more function calls, final text response generated
      const synthStep = addStep("Synthesizing diagnostic report & recommendations...");
      await new Promise((r) => setTimeout(r, 100));
      updateStep(synthStep, "completed", "Diagnosis and recommendations generated");

      const textParts = modelParts
        .filter((p: Record<string, unknown>) => typeof p.text === "string")
        .map((p: Record<string, unknown>) => p.text as string);

      const finalText = textParts.join("\n").trim() || "System diagnostics completed.";

      // Extract actionable fixes from collected tool data
      const fixes = this.extractActionableFixes(allCollectedToolResponses);
      callbacks?.onFixesIdentified?.(fixes);

      const diagnosticData = this.buildDiagnosticVisualData(userPrompt, allCollectedToolResponses);

      return {
        text: finalText,
        steps: [initStep, ...steps.filter((s) => s.id !== initStep.id)],
        fixes,
        diagnosticData,
      };
    }

    throw new Error("Reached maximum tool invocation iterations without completion.");
  }

  /**
   * High-accuracy native system diagnostic engine that runs locally using real application telemetry.
   */
  private async executeNativeDiagnosticEngine(
    userPrompt: string,
    steps: DiagnosticStep[],
    addStep: (label: string, toolName?: string) => DiagnosticStep,
    updateStep: (step: DiagnosticStep, status: "completed" | "error", detail?: string) => void,
    callbacks?: AgentExecutionCallbacks
  ): Promise<{
    text: string;
    steps: DiagnosticStep[];
    fixes: ActionableFix[];
    diagnosticData?: DiagnosticVisualData;
  }> {
    const lower = userPrompt.toLowerCase();

    // Specific category markers
    const hasStartupWord = lower.includes("startup") || lower.includes("boot") || lower.includes("autostart") || lower.includes("start up");
    const hasProcessOrRamWord = lower.includes("ram") || lower.includes("memory") || lower.includes("cpu") || lower.includes("process") || lower.includes("task") || lower.includes("background");
    const hasStorageWord = lower.includes("storage") || lower.includes("disk") || lower.includes("space") || lower.includes("drive") || lower.includes("cache") || lower.includes("temp") || lower.includes("clean");
    const hasGeneralSlowWord = lower.includes("slow") || lower.includes("lag") || lower.includes("optimize") || lower.includes("health") || lower.includes("audit") || lower.includes("recommend") || lower.includes("system check");

    let queryCategory: "startup_delay" | "process_memory" | "storage_space" | "general_slow" | "informational" = "general_slow";

    if (hasStartupWord && !hasProcessOrRamWord && !hasStorageWord && !hasGeneralSlowWord) {
      queryCategory = "startup_delay";
    } else if (hasProcessOrRamWord && !hasStartupWord && !hasStorageWord && !hasGeneralSlowWord) {
      queryCategory = "process_memory";
    } else if (hasStorageWord && !hasStartupWord && !hasProcessOrRamWord && !hasGeneralSlowWord) {
      queryCategory = "storage_space";
    } else if (hasGeneralSlowWord) {
      queryCategory = "general_slow";
    } else if (hasStartupWord) {
      queryCategory = "startup_delay";
    } else if (hasProcessOrRamWord) {
      queryCategory = "process_memory";
    } else if (hasStorageWord) {
      queryCategory = "storage_space";
    } else {
      queryCategory = "informational";
    }

    const collectedResults: ToolExecutionResponse[] = [];

    if (queryCategory === "startup_delay") {
      const startupStep = addStep("Checking Windows startup applications and boot timeline...", "check_startup_applications");
      const startupResult = await executeDiagnosticTool("check_startup_applications", { filter: "all" });
      collectedResults.push(startupResult);
      updateStep(startupStep, "completed", startupResult.summary);
    } else if (queryCategory === "process_memory") {
      const overviewStep = addStep("Checking active system load & strain...", "check_system_health_overview");
      const overviewResult = await executeDiagnosticTool("check_system_health_overview", {});
      collectedResults.push(overviewResult);
      updateStep(overviewStep, "completed", overviewResult.summary);

      const procStep = addStep("Inspecting background processes & memory footprint...", "check_background_processes");
      const procResult = await executeDiagnosticTool("check_background_processes", { filter: "attention", limit: 8 });
      collectedResults.push(procResult);
      updateStep(procStep, "completed", procResult.summary);
    } else if (queryCategory === "storage_space") {
      const storageStep = addStep("Analyzing drive capacity & category breakdown...", "check_storage_status");
      const storageResult = await executeDiagnosticTool("check_storage_status", {});
      collectedResults.push(storageResult);
      updateStep(storageStep, "completed", storageResult.summary);
    } else if (queryCategory === "general_slow") {
      const overviewStep = addStep("Checking system metrics & strain condition...", "check_system_health_overview");
      const overviewResult = await executeDiagnosticTool("check_system_health_overview", {});
      collectedResults.push(overviewResult);
      updateStep(overviewStep, "completed", overviewResult.summary);

      const startupStep = addStep("Evaluating startup applications & boot delay...", "check_startup_applications");
      const startupResult = await executeDiagnosticTool("check_startup_applications", { filter: "all" });
      collectedResults.push(startupResult);
      updateStep(startupStep, "completed", startupResult.summary);

      const procStep = addStep("Analyzing background processes & active threads...", "check_background_processes");
      const procResult = await executeDiagnosticTool("check_background_processes", { filter: "attention", limit: 8 });
      collectedResults.push(procResult);
      updateStep(procStep, "completed", procResult.summary);

      const storageStep = addStep("Checking storage headroom & file distribution...", "check_storage_status");
      const storageResult = await executeDiagnosticTool("check_storage_status", {});
      collectedResults.push(storageResult);
      updateStep(storageStep, "completed", storageResult.summary);
    }

    // Synthesis Step
    const synthStep = addStep("Synthesizing multi-step AI diagnosis & recommendations...");
    await new Promise((r) => setTimeout(r, 140));
    updateStep(synthStep, "completed", "Diagnosis and actionable steps ready");

    // Synthesize multi-step diagnosis and selective visual data payload
    const diagnosticData = this.buildDiagnosticVisualData(userPrompt, collectedResults);
    const text = this.synthesizeLocalReport(userPrompt, collectedResults, queryCategory);
    const fixes = this.extractActionableFixes(collectedResults);
    callbacks?.onFixesIdentified?.(fixes);

    return {
      text,
      steps: [...steps],
      fixes,
      diagnosticData,
    };
  }

  /**
   * Synthesize a comprehensive, multi-step conversational AI report.
   */
  private synthesizeLocalReport(
    query: string,
    toolResults: ToolExecutionResponse[],
    queryCategory: "startup_delay" | "process_memory" | "storage_space" | "general_slow" | "informational"
  ): string {
    const startupData = toolResults.find((r) => r.toolName === "check_startup_applications")?.data as Record<string, unknown> | undefined;
    const procData = toolResults.find((r) => r.toolName === "check_background_processes")?.data as Record<string, unknown> | undefined;
    const overviewData = toolResults.find((r) => r.toolName === "check_system_health_overview")?.data as Record<string, unknown> | undefined;
    const storageData = toolResults.find((r) => r.toolName === "check_storage_status")?.data as Record<string, unknown> | undefined;

    switch (queryCategory) {
      case "startup_delay":
        return this.synthesizeStartupReport(startupData);
      case "process_memory":
        return this.synthesizeProcessReport(procData, overviewData);
      case "storage_space":
        return this.synthesizeStorageReport(storageData);
      case "general_slow":
        return this.synthesizeGeneralSlowReport(overviewData, startupData, procData, storageData);
      case "informational":
      default:
        return this.synthesizeInformationalReport(query);
    }
  }

  private synthesizeStartupReport(startupData?: Record<string, unknown>): string {
    if (!startupData) {
      return "I was unable to retrieve Windows startup entries. Please verify permissions or check the Startup Manager directly.";
    }

    const totalCount = Number(startupData.totalStartupItems) || 0;
    const delayMs = Number(startupData.estimatedBootDelayMs) || 0;
    const delaySec = (delayMs / 1000).toFixed(1);
    const highImpact = (startupData.highImpactItems as Array<Record<string, unknown>>) || [];
    const disabledCount = Number(startupData.disabledCount) || 0;

    const topItemsList = highImpact.slice(0, 4).map((item) => {
      const name = String(item.name || "Unknown");
      const duration = Number(item.bootDurationMs) || 0;
      const cpu = Number(item.bootCpuMs) || 0;
      const runningTag = item.isCurrentlyRunning ? " (currently active)" : "";
      return `* **${name}**: Adds **~${duration.toLocaleString()} ms** to boot (${cpu} ms CPU time)${runningTag}.`;
    }).join("\n");

    const topNames = highImpact.slice(0, 2).map((i) => `\`${i.name}\``).join(" and ") || "heavy startup programs";

    return `### Executive Boot-Time Diagnosis
Your computer is registering **${totalCount} startup items**, contributing to an estimated **${delaySec} seconds of boot-time delay**. Of these, **${highImpact.length} applications** have a heavy performance footprint that delays Windows from presenting an interactive, responsive desktop upon login (${disabledCount} already disabled).

### High-Impact Contributors
${topItemsList || "* No severe startup delay bottlenecks were detected among enabled entries."}

### Multi-Step Boot Optimization Plan
1. **Disable non-essential autostarts:** Applications like ${topNames} do not strictly need to launch with Windows. Disabling them preserves quick cold boot times while keeping them fully usable when opened manually.
2. **Shift heavy apps to on-demand launching:** Review launchers, communication tools, and media updaters. Use the 1-click **Disable Startup** button below or manage them in the Startup Manager.
3. **Verify Windows Fast Startup & UEFI Boot:** Ensure Fast Startup is enabled in Windows Control Panel (Power Options) so the system hibernates the core kernel session for expedited boot times.`;
  }

  private synthesizeProcessReport(
    procData?: Record<string, unknown>,
    overviewData?: Record<string, unknown>
  ): string {
    const strain = Number(overviewData?.systemStrainPercent) || 0;
    const condition = String(overviewData?.condition || "normal");
    const ramPercent = Number(overviewData?.memoryUsagePercent) || 0;
    const memUsed = String(overviewData?.memoryUsed || "—");
    const memTotal = String(overviewData?.memoryTotal || "—");
    const bgLoad = Number(procData?.backgroundLoadPercent) || 0;
    const fgLoad = Number(procData?.foregroundLoadPercent) || 0;
    const fgName = String(overviewData?.foregroundProcessName || "Active application");

    const topProcs = (procData?.topProcesses as Array<Record<string, unknown>>) || [];
    const procList = topProcs.slice(0, 4).map((p) => {
      const name = String(p.name || "Unknown");
      const pid = Number(p.pid) || 0;
      const cpu = Number(p.sustainedCpuPercent || p.cpuPercent || 0).toFixed(1);
      const mem = String(p.memoryFormatted || "—");
      const flag = p.isSystemCritical ? " [Protected System Process]" : "";
      return `* **${name}** (PID ${pid}): Consuming **${cpu}% CPU** and **${mem} RAM**${flag}.`;
    }).join("\n");

    const topNonCrit = topProcs.find((p) => !p.isSystemCritical);
    const topNonCritName = topNonCrit ? `\`${topNonCrit.name}\`` : "background tasks";

    return `### Executive Resource & Memory Analysis
Your system is currently operating with a **System Strain of ${strain}% (${condition})**. Memory allocation is at **${ramPercent.toFixed(0)}% (${memUsed} used out of ${memTotal})**. Active CPU utilization is split between **${fgLoad.toFixed(0)}% foreground** (${fgName}) and **${bgLoad.toFixed(0)}% background tasks**.

### High-Resource Background Processes
${procList || "* No abnormal CPU or memory thrashing was observed among running background tasks."}

### Multi-Step Remediation Plan
1. **Inspect non-critical resource consumers:** ${topNonCritName} is maintaining an elevated resource footprint. Click **Inspect Process** below to view real-time thread activity or terminate unresponsive instances.
2. **Mitigate multi-process browser & app bloat:** Modern web applications and electron tools spin up separate sub-processes for each tab and extension. Closing inactive browser tabs and disabling unnecessary background extensions can immediately free 1-2 GB of RAM.
3. **Check for background indexing loops:** File synchronization services, antivirus background scans, or continuous updater daemons can saturate background CPU cycles during active workflows.`;
  }

  private synthesizeStorageReport(storageData?: Record<string, unknown>): string {
    if (!storageData) {
      return "I was unable to query disk drives. Please ensure storage permissions are granted.";
    }

    const drives = (storageData.drives as Array<Record<string, unknown>>) || [];
    const primaryDrive = drives[0];
    const driveName = String(primaryDrive?.drive || "C:");
    const usedPct = Number(primaryDrive?.usagePercentage || 0).toFixed(0);
    const usedSpace = String(primaryDrive?.used || "—");
    const freeSpace = String(primaryDrive?.free || "—");
    const totalSpace = String(primaryDrive?.total || "—");

    const categories = (storageData.categories as Array<Record<string, unknown>>) || [];
    const catList = categories.slice(0, 4).map((c) => {
      const label = String(c.label || c.category || "Other");
      const size = String(c.sizeFormatted || "—");
      const pct = Number(c.percentage || 0).toFixed(0);
      return `* **${label}**: **${size}** (${pct}% of volume)`;
    }).join("\n");

    const largestFiles = (storageData.largestFiles as Array<Record<string, unknown>>) || [];
    const fileList = largestFiles.slice(0, 3).map((f) => {
      const name = String(f.name || "File");
      const size = String(f.sizeFormatted || "—");
      return `* \`${name}\` (${size})`;
    }).join("\n");

    return `### Executive Storage & Capacity Diagnosis
Drive **${driveName}** is currently at **${usedPct}% utilization**, with **${usedSpace} used** and **${freeSpace} free space remaining** out of **${totalSpace}**. ${
      Number(usedPct) >= 85
        ? "⚠️ **Warning:** Free space is critically low, which can degrade Windows virtual memory (pagefile), crash app updates, and reduce SSD lifespan."
        : "Storage headroom is currently sufficient for normal Windows operation."
    }

### Storage Composition & Large Hotspots
${catList || "* Category composition calculation in progress."}

${largestFiles.length > 0 ? `**Notable Large Files Identified:**\n${fileList}` : ""}

### Multi-Step Storage Reclamation Plan
1. **Purge temporary cache files & crash dumps:** Windows update cache, browser temp files, and shader caches can be pruned safely without affecting user files.
2. **Audit heavy downloads & obsolete installers:** Check the Downloads folder and user profile directories for large ISO images, installer setup files, or zip archives that are no longer needed.
3. **Maintain minimum 15% SSD provisioning:** SSDs require unallocated capacity for TRIM garbage collection and wear leveling to maintain maximum read/write throughput.`;
  }

  private synthesizeGeneralSlowReport(
    overviewData?: Record<string, unknown>,
    startupData?: Record<string, unknown>,
    procData?: Record<string, unknown>,
    storageData?: Record<string, unknown>
  ): string {
    const strain = Number(overviewData?.systemStrainPercent) || 0;
    const condition = String(overviewData?.condition || "normal");
    const ramPct = Number(overviewData?.memoryUsagePercent) || 0;
    const cpuPct = Number(overviewData?.cpuUsagePercent) || 0;

    const startupDelaySec = ((Number(startupData?.estimatedBootDelayMs) || 0) / 1000).toFixed(1);
    const highStartupCount = ((startupData?.highImpactItems as Array<unknown>) || []).length;

    const bgLoad = Number(procData?.backgroundLoadPercent) || 0;

    const primaryDrive = ((storageData?.drives as Array<Record<string, unknown>>) || [])[0];
    const diskPct = Number(primaryDrive?.usagePercentage || 0);

    // Identify primary bottleneck
    let primaryBottleneck = "Distributed background load";
    let bottleneckReason = "Multiple background factors are contributing to reduced system responsiveness.";

    if (ramPct >= 85) {
      primaryBottleneck = "Severe Memory (RAM) Saturation";
      bottleneckReason = `RAM usage is at ${ramPct.toFixed(0)}%, causing Windows to swap memory pages to disk, inducing perceptible lag.`;
    } else if (cpuPct >= 75 || bgLoad >= 40) {
      primaryBottleneck = "High CPU Background Load";
      bottleneckReason = `Background tasks are consuming ${bgLoad.toFixed(0)}% of processor capacity, competing directly with active apps.`;
    } else if (highStartupCount >= 3 && Number(startupDelaySec) >= 3.0) {
      primaryBottleneck = "Excessive Startup Application Overhead";
      bottleneckReason = `${highStartupCount} high-impact autostarts add ~${startupDelaySec}s to boot time and persist in the background.`;
    } else if (diskPct >= 90) {
      primaryBottleneck = "Critical Storage Exhaustion";
      bottleneckReason = `Drive ${primaryDrive?.drive || "C:"} is ${diskPct.toFixed(0)}% full, constraining system swap files and write caches.`;
    }

    return `### Multi-Point System Performance Diagnosis
I conducted a comprehensive audit across your processor load, active memory footprint, startup applications, and storage capacity. System Strain is currently rated **${strain}% (${condition})**.

### Primary Bottleneck: ${primaryBottleneck}
${bottleneckReason}

### Telemetry Summary Across System Layers
* **Processor Load:** CPU is at **${cpuPct.toFixed(0)}%**, with **${bgLoad.toFixed(0)}%** consumed by background tasks.
* **Memory Allocation:** RAM is at **${ramPct.toFixed(0)}%** (${overviewData?.memoryUsed || "—"} in use).
* **Startup Impact:** **${highStartupCount} high-impact apps** contribute **~${startupDelaySec}s** to total boot delay.
* **Storage Headroom:** Primary Drive is at **${diskPct.toFixed(0)}% capacity** (${primaryDrive?.free || "—"} free).

### Multi-Step Remediation Plan
1. **Apply immediate 1-click fixes:** Use the action cards below to disable heavy startup entries and inspect top resource-consuming background tasks.
2. **Trim background processes:** Close background instances of web browsers, chat applications, or launchers that you are not actively using.
3. **Preserve disk breathing room:** Free up at least 15-20% disk space on Drive C: to allow Windows pagefile and disk cache to function without latency.`;
  }

  private synthesizeInformationalReport(query: string): string {
    const lower = query.toLowerCase();

    if (lower.includes("boot") || lower.includes("startup")) {
      return `### Understanding Windows Boot Time & Optimization
Windows boot time measures the duration from BIOS/UEFI handover until all autostart background services and user-level applications initialize and reach an idle state.

### Key Factors Influencing Boot Performance
1. **Startup Application Quantity:** Every application in the Registry \`Run\` keys or Startup folder initiates its executable concurrently, bottlenecking CPU cores and disk I/O.
2. **Fast Startup (Hybrid Hibernation):** Windows Fast Startup saves the kernel state to \`hiberfil.sys\` on shutdown, enabling rapid cold boots.
3. **Storage Device Speed:** NVMe SSDs read at 3,500-7,000 MB/s compared to mechanical HDDs (100-150 MB/s), drastically compressing service start time.

### Multi-Step Optimization Advice
1. Keep enabled startup applications under 5 essential tools.
2. Move communication and media tools (Teams, Discord, Spotify, Steam) to on-demand manual launching.
3. Regularly audit Task Scheduler for legacy updater tasks left behind by uninstalled software.`;
    }

    if (lower.includes("ram") || lower.includes("memory") || lower.includes("virtual")) {
      return `### Understanding Windows Memory Architecture
Windows dynamically balances physical RAM with virtual memory (paging file) to maximize responsiveness while preventing out-of-memory system faults.

### How Windows Allocates RAM
* **In Use:** Active memory assigned to running apps, system drivers, and the OS kernel.
* **Standby (Cached):** Pages containing frequently used files and libraries kept in memory. If an app requests more memory, Windows immediately flushes standby pages.
* **Modified / Paged:** Inactive memory pages written to \`pagefile.sys\` on the SSD/HDD when physical RAM pressure rises.

### Multi-Step Memory Best Practices
1. **Avoid aggressive third-party RAM cleaners:** Modern Windows memory management is self-optimizing. Aggressive RAM flushes force Windows to re-read everything from disk, increasing stutter.
2. **Monitor browser tab count:** Use tab sleeping/suspension features to reduce background memory consumption.
3. **Keep paging file managed automatically:** Allow Windows to dynamically scale \`pagefile.sys\` on your fastest SSD.`;
    }

    return `### Windows System Performance Guide
Maintaining high system responsiveness on Windows relies on balancing background application overhead, memory headroom, and storage hygiene.

### Multi-Step Optimization Principles
1. **Minimize background autostart overhead:** Prevent background app bloat by auditing startup applications regularly.
2. **Maintain storage headroom:** Ensure your primary OS drive has at least 15-20% free space for virtual memory (pagefile), shadow copies, and SSD wear-leveling.
3. **Monitor active process resource consumption:** Inspect applications maintaining persistent high background CPU or memory footprint using ASAO Process Analyzer.`;
  }

  /**
   * Build structured visualization data for rich dashboard rendering in the chat.
   * Selectively populates widgets and pills based on the tools executed.
   */
  public buildDiagnosticVisualData(
    userPrompt: string,
    toolResults: ToolExecutionResponse[]
  ): DiagnosticVisualData | undefined {
    if (!toolResults || toolResults.length === 0) {
      return undefined;
    }

    const lower = userPrompt.toLowerCase();
    let queryType: DiagnosticVisualData["queryType"] = "general_slow";
    if (lower.includes("boot") || lower.includes("startup") || lower.includes("autostart")) {
      queryType = "startup_delay";
    } else if (lower.includes("ram") || lower.includes("memory") || lower.includes("cpu") || lower.includes("process")) {
      queryType = "process_memory";
    } else if (lower.includes("storage") || lower.includes("disk") || lower.includes("space") || lower.includes("drive")) {
      queryType = "storage_space";
    }

    const hasStartup = toolResults.some((r) => r.toolName === "check_startup_applications");
    const hasProcesses = toolResults.some((r) => r.toolName === "check_background_processes");
    const hasOverview = toolResults.some((r) => r.toolName === "check_system_health_overview");
    const hasStorage = toolResults.some((r) => r.toolName === "check_storage_status" || r.toolName === "check_storage_insights");

    const visibleWidgets: ("pills" | "resource_chart" | "process_chart" | "startup_chart" | "storage_card" | "actionable_fixes")[] = [];
    const visiblePills: ("cpu" | "ram" | "disk" | "startup")[] = [];

    if (hasStartup) {
      visiblePills.push("startup");
      visibleWidgets.push("startup_chart");
    }

    if (hasProcesses || hasOverview) {
      visiblePills.push("cpu", "ram");
      if (hasOverview) visibleWidgets.push("resource_chart");
      if (hasProcesses) visibleWidgets.push("process_chart");
    }

    if (hasStorage) {
      visiblePills.push("disk");
      visibleWidgets.push("storage_card");
    }

    if (visiblePills.length > 0) {
      visibleWidgets.unshift("pills");
    }

    visibleWidgets.push("actionable_fixes");

    const overviewData = toolResults.find((r) => r.toolName === "check_system_health_overview")?.data as
      | Record<string, unknown>
      | undefined;
    const startupData = toolResults.find((r) => r.toolName === "check_startup_applications")?.data as
      | Record<string, unknown>
      | undefined;
    const procData = toolResults.find((r) => r.toolName === "check_background_processes")?.data as
      | Record<string, unknown>
      | undefined;
    const storageData = toolResults.find((r) => r.toolName === "check_storage_status")?.data as
      | Record<string, unknown>
      | undefined;

    return {
      queryType,
      visibleWidgets,
      visiblePills,
      overview: overviewData
        ? {
            cpuUsagePercent: Number(overviewData.cpuUsagePercent) || 0,
            memoryUsagePercent: Number(overviewData.memoryUsagePercent) || 0,
            memoryUsed: String(overviewData.memoryUsed || "—"),
            memoryTotal: String(overviewData.memoryTotal || "—"),
            gpuUsagePercent: Number(overviewData.gpuUsagePercent) || 0,
            foregroundLoadPercent: Number(overviewData.foregroundLoadPercent) || 0,
            backgroundLoadPercent: Number(overviewData.backgroundLoadPercent) || 0,
            systemStrainPercent: Number(overviewData.systemStrainPercent) || 0,
            condition: String(overviewData.condition || "healthy"),
            conditionReason: String(overviewData.conditionReason || ""),
            foregroundProcessName: String(overviewData.foregroundProcessName || ""),
          }
        : undefined,
      startup: startupData
        ? {
            totalStartupItems: Number(startupData.totalStartupItems) || 0,
            highImpactCount: Number(startupData.highImpactCount) || 0,
            disabledCount: Number(startupData.disabledCount) || 0,
            runningCount: Number(startupData.runningCount) || 0,
            estimatedBootDelayMs: Number(startupData.estimatedBootDelayMs) || 0,
            highImpactItems: (startupData.highImpactItems as Array<any>) || [],
            sampledItems: (startupData.sampledItems as Array<any>) || [],
          }
        : undefined,
      processes: procData
        ? {
            totalProcesses: Number(procData.totalProcesses) || 0,
            backgroundLoadPercent: Number(procData.backgroundLoadPercent) || 0,
            foregroundLoadPercent: Number(procData.foregroundLoadPercent) || 0,
            systemStrainPercent: Number(procData.systemStrainPercent) || 0,
            topProcesses: (procData.topProcesses as Array<any>) || [],
          }
        : undefined,
      storage: storageData
        ? {
            selectedDrive: String(storageData.selectedDrive || "C:"),
            drives: (storageData.drives as Array<any>) || [],
            categories: (storageData.categories as Array<any>) || [],
            largestFiles: (storageData.largestFiles as Array<any>) || [],
            largestFolders: (storageData.largestFolders as Array<any>) || [],
          }
        : undefined,
    };
  }

  /**
   * Automatically generate 1-click ActionableFix objects from collected tool data.
   */
  private extractActionableFixes(toolResults: ToolExecutionResponse[]): ActionableFix[] {
    const fixes: ActionableFix[] = [];

    // 1. Startup Fixes
    const startupData = toolResults.find((r) => r.toolName === "check_startup_applications")?.data as
      | Record<string, unknown>
      | undefined;

    if (startupData) {
      const highImpact = (startupData.highImpactItems as Array<Record<string, unknown>>) || [];
      highImpact.slice(0, 2).forEach((item) => {
        if (item.isEnabled) {
          fixes.push({
            id: `fix-startup-${item.id}`,
            title: `Disable ${item.name} at Startup`,
            description: `Reclaim ~${item.bootDurationMs}ms boot time and eliminate background launch.`,
            actionType: "disable_startup",
            payload: {
              startupId: String(item.id),
              startupName: String(item.name),
              source: String(item.source),
            },
            status: "idle",
          });
        }
      });
    }

    // 2. Process Inspection Fixes
    const procData = toolResults.find((r) => r.toolName === "check_background_processes")?.data as
      | Record<string, unknown>
      | undefined;

    if (procData) {
      const topProcs = (procData.topProcesses as Array<Record<string, unknown>>) || [];
      const topNonCritical = topProcs.find((p) => !p.isSystemCritical);
      if (topNonCritical) {
        fixes.push({
          id: `fix-proc-${topNonCritical.pid}`,
          title: `Inspect ${topNonCritical.name}`,
          description: `View resource metrics and manage process in the Process Analyzer.`,
          actionType: "inspect_process",
          payload: {
            pid: Number(topNonCritical.pid),
            processName: String(topNonCritical.name),
            route: "processes",
          },
          status: "idle",
        });
      }
    }

    // 3. Storage Cleanup Fixes
    const storageData = toolResults.find((r) => r.toolName === "check_storage_status")?.data as
      | Record<string, unknown>
      | undefined;

    if (storageData) {
      const largestFiles = (storageData.largestFiles as Array<Record<string, unknown>>) || [];
      if (largestFiles.length > 0 && largestFiles[0].path) {
        const topFile = largestFiles[0];
        fixes.push({
          id: `fix-storage-${Date.now()}`,
          title: `Open Large File Location`,
          description: `Open folder containing ${topFile.name} (${topFile.sizeFormatted}) in File Explorer.`,
          actionType: "open_storage",
          payload: {
            path: String(topFile.path),
          },
          status: "idle",
        });
      }
    }

    return fixes;
  }

  private getToolStepDisplayLabel(toolName: string): string {
    switch (toolName) {
      case "check_startup_applications":
        return "Checking startup applications";
      case "check_background_processes":
        return "Checking background processes";
      case "check_storage_status":
        return "Checking storage";
      case "check_storage_insights":
        return "Checking storage cleanup opportunities";
      case "check_system_health_overview":
        return "Checking system metrics";
      default:
        return `Calling ${toolName}`;
    }
  }
}

export const vertexAdkAgentService = new VertexAdkAgentService();
