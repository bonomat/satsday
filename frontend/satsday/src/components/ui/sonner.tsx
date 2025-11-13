import { Toaster as Sonner, toast } from "sonner";

type ToasterProps = React.ComponentProps<typeof Sonner>;

const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      theme="dark"
      className="toaster group"
      position="top-center"
      toastOptions={{
        classNames: {
          toast:
            "group toast group-[.toaster]:bg-white group-[.toaster]:text-gray-900 group-[.toaster]:border-2 group-[.toaster]:border-orange-400 group-[.toaster]:shadow-2xl",
          description: "group-[.toast]:text-gray-700 group-[.toast]:font-medium",
          actionButton:
            "group-[.toast]:bg-orange-500 group-[.toast]:text-white group-[.toast]:font-bold",
          cancelButton:
            "group-[.toast]:bg-gray-200 group-[.toast]:text-gray-800",
          success: "group-[.toast]:bg-green-500 group-[.toast]:text-white group-[.toast]:border-green-400 group-[.toast]:font-bold",
          error: "group-[.toast]:bg-red-500 group-[.toast]:text-white group-[.toast]:border-red-400 group-[.toast]:font-bold",
        },
      }}
      {...props}
    />
  );
};

export { Toaster, toast };
