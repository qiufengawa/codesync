import { Search, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { forwardRef } from "react";

type Props = {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  className?: string;
};

export const SearchInput = forwardRef<HTMLInputElement, Props>(function SearchInput(
  { value, onChange, placeholder = "搜索 id / 标题 / 首条消息 / 目录", className },
  ref,
) {
  return (
    <div className={"relative " + (className ?? "")}>
      <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      <Input
        ref={ref}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="pl-8 pr-8"
      />
      {value && (
        <Button
          variant="ghost"
          size="icon"
          className="absolute right-1 top-1/2 h-7 w-7 -translate-y-1/2"
          onClick={() => onChange("")}
          aria-label="清除搜索"
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      )}
    </div>
  );
});
