import React from "react";
import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";

interface AppShellProps {
  children: React.ReactNode;
}

export const AppShell: React.FC<AppShellProps> = ({ children }) => {
  return (
    <div className="h-screen w-screen flex flex-col bg-transparent text-lunar-text overflow-hidden">
      <Topbar />
      <div className="flex-1 flex min-h-0 overflow-hidden">
        <Sidebar />
        <main className="flex-1 flex flex-col min-w-0 min-h-0 overflow-hidden bg-transparent">
          {children}
        </main>
      </div>
    </div>
  );
};
