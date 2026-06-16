import { FileText, FolderOpen, Loader, RotateCcw, Save } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { useAppState } from '@/hooks/use-app-state';
import useTools from '@/hooks/use-tools';
import { RunningState } from '@/types/app-state';

interface ToolsGroupProps {
  getDisabled: (state: RunningState, disableOnRunning?: boolean) => boolean;
}

export function ToolsGroup({ getDisabled }: ToolsGroupProps) {
  const { runningState } = useAppState();
  const { exporting, exportTranscript, openPath, revealPath, setPlaceholderData } = useTools();

  const onResetAll = async () => {
    try {
      await setPlaceholderData();
    } catch (error) {
      console.error(error);
      toast.error('Failed to reset');
    }
  };

  const onOpenFile = async (path: string) => {
    try {
      await openPath(path);
    } catch (error) {
      console.error(error);
      toast.error('Failed to open file');
    }
  };

  const onOpenFolder = async (path: string) => {
    try {
      await revealPath(path);
    } catch (error) {
      console.error(error);
      toast.error('Failed to open folder');
    }
  };

  const onExportTranscript = async () => {
    try {
      const filePath = await exportTranscript();
      // No path means the user cancelled the save dialog, so nothing was written.
      if (!filePath) {
        return;
      }
      toast.success('Interview exported successfully', {
        // Give the user time to use the open buttons before the toast dismisses.
        duration: 10000,
        action: (
          <div className="flex items-center ml-auto gap-1">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 cursor-pointer"
                  onClick={() => onOpenFile(filePath)}
                >
                  <FileText className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>Open file</p>
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 cursor-pointer"
                  onClick={() => onOpenFolder(filePath)}
                >
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>Open folder</p>
              </TooltipContent>
            </Tooltip>
          </div>
        ),
      });
    } catch (error) {
      console.error(error);
      toast.error(error instanceof Error ? error.message : 'Failed to export interview');
    }
  };

  return (
    <div className="flex items-center space-x-2">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="secondary"
            onClick={onResetAll}
            size="sm"
            className="h-8 w-8 text-xs rounded-xl cursor-pointer"
            disabled={getDisabled(runningState) || exporting}
          >
            <RotateCcw className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <p>Reset All</p>
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="secondary"
            onClick={onExportTranscript}
            size="sm"
            className="h-8 w-8 text-xs rounded-xl cursor-pointer"
            disabled={getDisabled(runningState) || exporting}
          >
            {exporting ? <Loader className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <p>Export Interview</p>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
